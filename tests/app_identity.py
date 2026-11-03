"""Reject stale/malformed app pairs even when version and audio checks agree."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import sys
import tempfile
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from app_identity import verify_pair

with tempfile.TemporaryDirectory(prefix='gamepkg-identity-test-') as temporary:
    root = Path(temporary)
    source = root / 'fixture.rs'
    source.write_text('''fn main() {
    let gui = std::env::current_exe().unwrap().file_name().unwrap().to_string_lossy().contains("gui");
    assert_eq!(std::env::args().nth(1).unwrap(), if gui { "--check-build-identity" } else { "check-build-identity" });
    assert!(std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none());
    assert_eq!(std::fs::read(std::env::var_os("GAMEPKG_HOME").unwrap()).unwrap(), b"untouched");
    println!("{}", std::env::var(if gui { "GAMEPKG_TEST_GUI_IDENTITY" } else { "GAMEPKG_TEST_CLI_IDENTITY" }).unwrap());
}''')
    suffix = '.exe' if sys.platform == 'win32' else ''
    cli = root / ('gamepkg' + suffix)
    subprocess.run(['rustc', '--edition=2021', str(source), '-o', str(cli)], check=True)
    shutil.copyfile(cli, root / ('gamepkg-gui' + suffix))
    record = {'schema': 1, 'version': '1.2.3',
              'os': 'windows' if sys.platform == 'win32' else 'macos' if sys.platform == 'darwin' else 'linux',
              'architecture': 'x86', 'native_sdk': '1234567890abcdef'}
    def check(gui, *, version='1.2.3', architecture='i686', cli_record=record):
        with patch.dict(os.environ, GAMEPKG_TEST_CLI_IDENTITY=json.dumps(cli_record),
                        GAMEPKG_TEST_GUI_IDENTITY=gui if isinstance(gui, str) else json.dumps(gui),
                        DISPLAY='must-not-connect', WAYLAND_DISPLAY='must-not-connect'):
            return verify_pair(root, version=version, architecture=architecture)
    result = check(record)
    assert result['identity'] == record and len(result['executables_sha256']) == 2
    rejected = 0
    for gui in [dict(record, native_sdk='abcdef1234567890'), dict(record, version='1.2.2'),
                dict(record, architecture='x86_64'), dict(record, schema=True),
                dict(record, native_sdk='../not-a-recipe'), dict(record, extra='unknown'),
                {key: value for key, value in record.items() if key != 'native_sdk'}, '{invalid json']:
        try:
            check(gui)
        except ValueError:
            rejected += 1
        else:
            raise AssertionError(f'Accepted incompatible app pair: {gui}')
    assert rejected == 8
print('App identity passed: relocated matching pair and eight incompatible/malformed pairs; same-version stale recipes rejected.')
