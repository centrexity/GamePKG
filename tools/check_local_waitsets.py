#!/usr/bin/env python3
"""Maintainer-only SDK qualification; Python is not an end-user dependency."""
import argparse, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path
parser = argparse.ArgumentParser(description='Maintainer native mixed-object wait components and required-failing variants; no title playback.')
parser.add_argument('--evidence', type=Path, required=True)
parser.add_argument('--wine', action='store_true', help='Execute Windows component binaries through Wine; otherwise build-only.')
args = parser.parse_args()
app = Path(__file__).resolve().parent.parent
out = args.evidence.resolve()
out.mkdir(exist_ok=False)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
files = [*sorted((app / 'engines/native-services').glob('*.rs')), app / 'include/gamepkg_port.h', *[app / 'tests' / n for n in ['local_runtime.rs', 'local_bindings.rs', 'local_semaphore_client.rs', 'local_waitset_client.rs']]]
hashes = {str(p.relative_to(app)): sha(p) for p in files}
results = []
with tempfile.TemporaryDirectory(prefix='gamepkg-waitset-native-') as t:
    temp = Path(t)
    for target in ['x86_64-unknown-linux-gnu', 'i686-unknown-linux-gnu', 'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu']:
        for suite in ['local_runtime', 'local_bindings', 'local_semaphore_client', 'local_waitset_client']:
            print('RUN', target, suite, flush=True)
            exe = temp / (target + '-' + suite + ('.exe' if 'windows' in target else ''))
            command = ['rustc', '--edition=2021', '--test', '-D', 'warnings', '--target', target, str(app / 'tests' / (suite + '.rs')), '-o', str(exe)]
            if target == 'i686-unknown-linux-gnu':
                command += ['-C', 'linker=gcc-11', '-C', 'link-arg=-lssp_nonshared']
            log = out / (target + '-' + suite + '.log')
            with log.open('w') as f:
                subprocess.run(command, check=True, stdout=f, stderr=subprocess.STDOUT)
                if 'windows' in target and (not args.wine):
                    results.append({'target': target, 'suite': suite, 'execution': 'build-only', 'log_sha256': sha(log)})
                    continue
                prefix = ['wine', str(exe)] if 'windows' in target else [str(exe)]
                subprocess.run([*prefix, '--test-threads=1'], check=True, timeout=120, stdout=f, stderr=subprocess.STDOUT, env=dict(os.environ, WINEDEBUG='-all', WINEPREFIX='/tmp/gamepkg-port-replace-wine'))
            counts = re.findall('test result: ok\\. (\\d+) passed;', log.read_text())
            assert len(counts) == 1
            results.append({'target': target, 'suite': suite, 'passed': int(counts[0]), 'execution': 'Wine' if 'windows' in target else 'native Linux', 'log_sha256': sha(log)})
    sdk = temp / 'sdk'
    shutil.copytree(app / 'engines/native-services', sdk)
    originals={name:(sdk/name).read_text() for name in ['local_runtime.rs','local_bindings.rs','client.rs']}
    variants=[]
    for name,old,new in [
        ('wait-all-as-any','.all(|ready| *ready)','.any(|ready| *ready)'),
        ('reverse-caller-priority','.position(|ready| *ready)','.rposition(|ready| *ready)'),
        ('do-not-reset-event','*signaled = false;','*signaled = true;'),
        ('do-not-consume-count','*count -= 1;','*count -= 0;'),
        ('do-not-own-mutex','*depth += 1;','*depth += 0;'),
        ('hide-abandonment','abandoned: abandoned_index.is_some(),','abandoned: false,'),
        ('uncancellable-set','let cancelled = self.task_cancellation()?;','let cancelled: Option<Arc<AtomicBool>> = None;'),
        ('ignore-revoked-set-object','if object.closed.load(Ordering::Acquire) {','if false && object.closed.load(Ordering::Acquire) {'),
        ('eager-task-completion','handle.as_ref().ok_or(Error::Denied)?.is_finished()','{let _ = handle.as_ref().ok_or(Error::Denied)?; true}'),
        ('forge-semaphore-kind','(3, Kind::Semaphore { count, .. })','(1 | 2 | 3, Kind::Semaphore { count, .. })'),
        ('overflow-after-consumption','depth.checked_add(1).ok_or(Error::Busy)?;','let _ = depth.checked_add(1);'),
    ]: variants.append((name,'local_runtime.rs','local_runtime','    pub fn wait_many(', '    pub fn new(', old,new))
    variants.append(('forget-abandoned-owner','local_runtime.rs','local_runtime','    fn task_exited(', '    pub fn wait_many(', '*abandoned = true;', '*abandoned = false;'))
    for name,old,new in [
        ('short-capacity-mutation','if capacity < 8 {','if false && capacity < 8 {'),
        ('accept-trailing-wait-request','r.end()?;', 'let _ = &r;'),
        ('accept-noncanonical-wait-mode','let all = r.boolean()?;', 'let all = r.u32()? != 0;'),
        ('discard-wait-deadline','let milliseconds = r.u32()?;', 'let _ = r.u32()?; let milliseconds = 0;'),
    ]:variants.append((name,'local_bindings.rs','local_bindings','        47 => {','        43 => {',old,new))
    for name,old,new in [
        ('accept-short-wait-reply','if local_call(47, &request, &mut output)? != 8 {','if local_call(47, &request, &mut output)? > 8 {'),
        ('accept-invalid-wait-reason','|| reason > 1','|| reason > 2'),
        ('abandoned-event-reply','objects[index as usize].kind != 2','objects[index as usize].kind == 0'),
        ('nonzero-normal-wait-all-index','all && reason == 0 && index != 0','all && reason == 0 && index > 1'),
    ]:variants.append((name,'client.rs','local_waitset_client','pub fn local_wait_many(', '/// Current reviewed task ID',old,new))
    for name,file,driver,begin,end,old,new in variants:
        for member,text in originals.items():(sdk/member).write_text(text)
        text=originals[file];start=text.index(begin);stop=text.index(end,start);body=text[start:stop];assert body.count(old)==1,(name,body.count(old))
        (sdk/file).write_text(text[:start]+body.replace(old,new,1)+text[stop:])
        source=temp/(name+'.rs');source.write_text((app/'tests'/(driver+'.rs')).read_text().replace('../engines/native-services/',str(sdk)+'/'))
        exe=temp/name;log=out/(name+'.log')
        with log.open('w')as f:
            subprocess.run(['rustc','--edition=2021','--test','-D','warnings',str(source),'-o',str(exe)],check=True,stdout=f,stderr=subprocess.STDOUT)
            result=subprocess.run([str(exe),'--test-threads=1'],timeout=120,stdout=f,stderr=subprocess.STDOUT)
        assert result.returncode!=0 and 'test result: FAILED.'in log.read_text(),name
        print('PASS required-failing',name,flush=True)
    assert hashes == {str(p.relative_to(app)): sha(p) for p in files}, 'sources changed during qualification'
    (out / 'qualification.json').write_text(json.dumps({'schema': 1, 'scope': 'native SDK components; Windows through Wine' if args.wine else 'native Linux components; Windows build-only', 'tool_sha256': sha(Path(__file__)), 'sources': hashes, 'results': results, 'required_failing_copies': len(variants), 'status': 'passed', 'game_admission': 'separate signed fixture required', 'full_native_profiles': 'remain blocked'}, indent=2) + '\n')
print('PASS four-target component builds and required-failing copies; execution scope is recorded per target', flush=True)
