#!/usr/bin/env python3
"""Maintainer-only SDK qualification; Python is not an end-user dependency."""
import argparse, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path
parser = argparse.ArgumentParser(description='Maintainer native non-consuming mutex-state components and required-failing variants; no title playback.')
parser.add_argument('--evidence', type=Path, required=True)
parser.add_argument('--wine', action='store_true', help='Execute Windows component binaries through Wine; otherwise build-only.')
args = parser.parse_args()
app = Path(__file__).resolve().parent.parent
out = args.evidence.resolve()
out.mkdir(exist_ok=False)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
files = [*sorted((app / 'engines/native-services').glob('*.rs')), app / 'include/gamepkg_port.h', *[app / 'tests' / n for n in ['local_runtime.rs', 'local_bindings.rs', 'local_mutex_state.rs', 'local_mutex_client.rs']]]
hashes = {str(p.relative_to(app)): sha(p) for p in files}
results = []
with tempfile.TemporaryDirectory(prefix='gamepkg-mutex-state-native-') as t:
    temp = Path(t)
    for target in ['x86_64-unknown-linux-gnu', 'i686-unknown-linux-gnu', 'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu']:
        for suite in ['local_runtime', 'local_bindings', 'local_mutex_state', 'local_mutex_client']:
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
        ('snapshot-acquires-mutex','        let object = self.sync_get(handle)?;','        self.mutex_lock(handle,0,true)?; let object = self.sync_get(handle)?;'),
        ('snapshot-unlocks-mutex','        let object = self.sync_get(handle)?;','        self.unlock(handle)?; let object = self.sync_get(handle)?;'),
        ('forge-caller-ownership','owned_by_caller: owner == Some(std::thread::current().id()),','owned_by_caller: owner.is_some(),'),
        ('hide-abandonment','            abandoned,\n            depth,\n        })','            abandoned: false,\n            depth,\n        })'),
        ('erase-depth','            depth,\n        })','            depth: 0,\n        })'),
        ('uncancellable-observation','let cancelled = self.task_cancellation()?;','let cancelled: Option<Arc<AtomicBool>> = None;'),
        ('ignore-retained-revocation','object.closed.load(Ordering::Acquire)','false'),
        ('accept-inconsistent-internal-state','if owner.is_some() != (depth != 0) || (abandoned && owner.is_some()) {','if false && (owner.is_some() != (depth != 0) || (abandoned && owner.is_some())) {'),
    ]:variants.append((name,'local_runtime.rs','local_mutex_state','    pub fn mutex_state(', '    fn wait_sync(', old,new))
    variants.append(('accept-trailing-state-request','local_bindings.rs','local_mutex_state','        48 => {','        47 => {','            r.end()?;','            let _ = &r;'))
    variants.append(('retirement-forgets-kind','local_runtime.rs','local_mutex_state','    pub fn release_sync_typed(', '    pub fn release_sync(', 'if !matches!(', 'if false && !matches!('))
    variants.append(('retirement-destroys-held-mutex','local_runtime.rs','local_mutex_state','    pub fn release_sync_typed(', '    pub fn release_sync(', 'if matches!(*value,Kind::Mutex{depth,..} if depth!=0)', 'if false && matches!(*value,Kind::Mutex{depth,..} if depth!=0)'))
    variants.append(('retirement-forgets-notification','local_runtime.rs','local_mutex_state','    pub fn release_sync_typed(', '    pub fn release_sync(', '        self.notify_waiters();', '        // Deliberately drop the wait-set notification.'))
    variants.append(('retirement-denies-cancelled-cleanup','local_runtime.rs','local_mutex_state','    pub fn release_sync_typed(', '    pub fn release_sync(', '        let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;', '        if self.task_cancellation()?.as_ref().is_some_and(|flag|flag.load(Ordering::Acquire)){return Err(Error::Denied);} let mut state = self.0.state.lock().map_err(|_| Error::Failed)?;'))
    variants.append(('retirement-accepts-trailing-request','local_bindings.rs','local_mutex_state','        49 => {','        48 => {','            r.end()?;','            let _ = &r;'))
    for name,old,new in [
        ('short-state-reply','if local_call(48, &mutex.to_le_bytes(), &mut output)? != 16 {','if local_call(48, &mutex.to_le_bytes(), &mut output)? > 16 {'),
        ('accept-nonboolean-locked','locked > 1','locked > 2'),
        ('accept-nonboolean-owned','owned > 1','owned > 2'),
        ('accept-nonboolean-abandoned','abandoned > 1','abandoned > 2'),
        ('allow-unlocked-owner','|| (owned != 0 && locked == 0)','|| false'),
        ('allow-depth-state-mismatch','|| (locked != 0) != (depth != 0)','|| false'),
        ('allow-owned-abandoned','|| (abandoned != 0 && locked != 0)','|| false'),
    ]:variants.append((name,'client.rs','local_mutex_client','pub fn local_mutex_state(', '#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub struct LocalSemaphoreState',old,new))
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
    (out / 'qualification.json').write_text(json.dumps({'schema': 1, 'scope': 'native SDK components; Windows through Wine' if args.wine else 'native Linux components; Windows build-only', 'tool_sha256': sha(Path(__file__)), 'sources': hashes, 'results': results, 'required_failing_copies': len(variants), 'status': 'passed', 'game_admission': 'separate signed fixture required', 'full_native_profiles': 'remain blocked', 'wire':'op 48: ID u64 LE, reply locked/owned_by_caller/abandoned/depth u32 LE (16 bytes)', 'observation':'no acquisition, release or abandonment consumption; copied state is not a lease'}, indent=2) + '\n')
print('PASS four-target component builds and required-failing copies; execution scope is recorded per target', flush=True)
