#!/usr/bin/env python3
"""Maintainer-only SDK qualification; Python is not an end-user dependency."""
import argparse, hashlib, json, os, re, shutil, subprocess, tempfile
from pathlib import Path
parser = argparse.ArgumentParser(description='Maintainer native semaphore components and required-failing variants; no title playback.')
parser.add_argument('--evidence', type=Path, required=True)
parser.add_argument('--wine', action='store_true', help='Execute Windows component binaries through Wine; otherwise build-only.')
args = parser.parse_args()
app = Path(__file__).resolve().parent.parent
out = args.evidence.resolve()
out.mkdir(exist_ok=False)

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
files = [*sorted((app / 'engines/native-services').glob('*.rs')), app / 'include/gamepkg_port.h', *[app / 'tests' / n for n in ['local_runtime.rs', 'local_bindings.rs', 'local_semaphore_client.rs']]]
hashes = {str(p.relative_to(app)): sha(p) for p in files}
results = []
with tempfile.TemporaryDirectory(prefix='gamepkg-semaphore-native-') as t:
    temp = Path(t)
    for target in ['x86_64-unknown-linux-gnu', 'i686-unknown-linux-gnu', 'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu']:
        for suite in ['local_runtime', 'local_bindings', 'local_semaphore_client']:
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
    original = (sdk / 'local_runtime.rs').read_text()
    source = temp / 'mutation.rs'
    source.write_text((app / 'tests/local_runtime.rs').read_text().replace('../engines/native-services/local_runtime.rs', str(sdk / 'local_runtime.rs')))
    mutations = [('do-not-consume-permit', '*count -= 1;', '*count -= 0;'), ('forget-release-count', '*count = next;', 'let _ = next; *count = previous;'), ('ignore-maximum', 'permits != 0 && next <= maximum', 'permits != 0 && (next <= maximum || *maximum != 0)'), ('accept-zero-release', 'permits != 0 && next <= maximum', 'next <= maximum'), ('legacy-untyped-semaphore-wait', 'if expected != WaitKind::Semaphore {', 'if expected == WaitKind::Mutex {'), ('uncancellable-semaphore-wait', 'let cancelled = if expected != WaitKind::Any {', 'let cancelled = if expected != WaitKind::Any && expected != WaitKind::Semaphore {')]
    for name, old, new in mutations:
        start = original.index('    fn wait_sync(') if name == 'do-not-consume-permit' else 0
        end = original.index('    pub fn unlock(', start) if name == 'do-not-consume-permit' else len(original)
        body = original[start:end]
        assert body.count(old) == 1, (name, body.count(old))
        (sdk / 'local_runtime.rs').write_text(original[:start] + body.replace(old, new, 1) + original[end:])
        exe = temp / name
        log = out / (name + '.log')
        with log.open('w') as f:
            subprocess.run(['rustc', '--edition=2021', '--test', '-D', 'warnings', str(source), '-o', str(exe)], check=True, stdout=f, stderr=subprocess.STDOUT)
            result = subprocess.run([str(exe), '--test-threads=1'], timeout=120, stdout=f, stderr=subprocess.STDOUT)
        assert result.returncode != 0 and 'test result: FAILED.' in log.read_text(), name
        print('PASS required-failing', name, flush=True)
    assert hashes == {str(p.relative_to(app)): sha(p) for p in files}, 'sources changed during qualification'
    (out / 'qualification.json').write_text(json.dumps({'schema': 1, 'scope': 'native SDK components; Windows through Wine' if args.wine else 'native Linux components; Windows build-only', 'tool_sha256': sha(Path(__file__)), 'sources': hashes, 'results': results, 'required_failing_runtime_copies': len(mutations), 'status': 'passed', 'game_admission': 'separate signed fixture required', 'full_native_profiles': 'remain blocked'}, indent=2) + '\n')
print('PASS four-target component builds and required-failing copies; execution scope is recorded per target', flush=True)
