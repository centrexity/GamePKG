"""Maintainer-only execution of the production controller timing tests at either width."""
import argparse
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit', action='store_true', dest='bits32')
parser.add_argument('--linker', default='gcc')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
source = (project / 'src/gui/input.rs').read_text()
start = source.index('#[derive(Default)]\nstruct LookMotion')
end = source.index('#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]', start)
production = source[start:end]
start = source.index('    #[test]\n    fn controller_turning_uses_time')
end = source.index('    use super::*;', start)
tests = source[start:end].replace('super::LookMotion', 'LookMotion')
assert tests.count('#[test]') == 2
with tempfile.TemporaryDirectory(prefix='gamepkg-controller-look-') as temporary:
    root = Path(temporary)
    rust = root / 'look.rs'
    binary = root / 'look'
    rust.write_text(production + '\n' + tests)
    command = ['rustc', '--edition=2021', '--test', '-Dwarnings', '-C', 'opt-level=2']
    if args.bits32:
        command += ['--target', 'i686-unknown-linux-gnu', '-C', 'linker=' + args.linker]
    subprocess.run(command + [str(rust), '-o', str(binary)], check=True)
    header = binary.read_bytes()[:6]
    assert header[:4] == b'\x7fELF'
    assert header[4] == (1 if args.bits32 else 2), 'Unexpected executable width'
    result = subprocess.run([str(binary)], capture_output=True, text=True, check=True, timeout=30)
    assert 'test result: ok. 2 passed; 0 failed; 0 ignored;' in result.stdout, result.stdout
    print(result.stdout, end='')
    print('Production controller look timing passed at pointer width', 32 if args.bits32 else 64)
