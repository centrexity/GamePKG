"""Execute the production touch gesture and its test against real egui at either width."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit', action='store_true', dest='bits32')
parser.add_argument('--linker', default='gcc')
parser.add_argument('--cancellation', action='store_true', help='run production cancellation/widget tests')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
source = (project / 'src/gui/input.rs').read_text()
start = source.index('#[derive(Default)]\nstruct TouchLook')
end = source.index('/// Stick values', start)
production = source[start:end]
start = source.index('    fn camera_touch(')
end = source.index('    #[test]\n    fn touch_camera_invalid_input', start)
tests = source[start:end]
assert tests.count('#[test]') == 1
expected_tests=1
if args.cancellation:
    tests=(project/'tests/desktop_touch_cancellation.rs').read_text()
    prefix='#![cfg(feature = "gui")]\n'
    assert tests.startswith(prefix)
    tests=tests[len(prefix):].replace('use eframe::egui::','use egui::')
    production=''
    expected_tests=3
    assert tests.count('#[test]') == expected_tests
with tempfile.TemporaryDirectory(prefix='gamepkg-touch-look-') as temporary:
    root = Path(temporary)
    (root / 'lib.rs').write_text(production + '\n' + tests)
    features=',features=["default_fonts"]' if args.cancellation else ''
    (root / 'Cargo.toml').write_text('[package]\nname="gamepkg-touch-look-proof"\nversion="0.0.0"\nedition="2021"\n[lib]\npath="lib.rs"\n[dependencies]\negui={version="=0.31.1",default-features=false'+features+'}\n[patch.crates-io]\negui={path='+json.dumps(str(project / 'vendor/egui'))+'}\n')
    (root / 'Cargo.lock').write_bytes((project / 'Cargo.lock').read_bytes())
    environment = dict(os.environ, CARGO_TARGET_DIR=str(project / 'target/touch-look-proof'), RUSTFLAGS='-Dwarnings')
    command = ['cargo', 'test', '--offline', '--manifest-path', str(root / 'Cargo.toml'), '--lib', '--no-run', '--message-format=json']
    if args.bits32:
        command += ['--target', 'i686-unknown-linux-gnu']
        environment['CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_LINKER'] = args.linker
    built = subprocess.run(command, env=environment, capture_output=True, text=True, timeout=180)
    if built.returncode:
        raise RuntimeError(built.stderr + built.stdout[-4000:])
    artifacts = [json.loads(line) for line in built.stdout.splitlines() if line.startswith('{')]
    assert any(item.get('manifest_path') == str(project/'vendor/egui/Cargo.toml') for item in artifacts), 'Production patched egui was not compiled'
    executables = [Path(item['executable']) for item in artifacts if item.get('executable') and item.get('reason') == 'compiler-artifact']
    assert len(executables) == 1, executables
    header = executables[0].read_bytes()[:6]
    assert header[:4] == b'\x7fELF' and header[4] == (1 if args.bits32 else 2)
    result = subprocess.run([str(executables[0])], capture_output=True, text=True, check=True, timeout=30)
    assert f'test result: ok. {expected_tests} passed; 0 failed; 0 ignored;' in result.stdout, result.stdout
    print(result.stdout, end='')
    print('Production touch', 'cancellation' if args.cancellation else 'gesture', 'passed with real egui at pointer width', 32 if args.bits32 else 64)
