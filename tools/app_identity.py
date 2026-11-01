"""Verify that relocated CLI/GUI entries use the same native cache recipe."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tempfile


def verify_pair(binary_directory, *, version=None, system=None, architecture=None):
    system = system or platform.system()
    suffix = '.exe' if system == 'Windows' else ''
    records = []
    hashes = {}
    with tempfile.TemporaryDirectory(prefix='gamepkg-build-identity-') as temporary:
        root = Path(temporary)
        library = root / 'not-a-library'
        config = root / 'invalid-settings.toml'
        library.write_bytes(b'untouched')
        config.write_text('deliberately invalid settings\n')
        env = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(config))
        env.pop('DISPLAY', None)
        env.pop('WAYLAND_DISPLAY', None)
        for name, option in [('gamepkg', 'check-build-identity'),
                             ('gamepkg-gui', '--check-build-identity')]:
            source = Path(binary_directory) / (name + suffix)
            if source.is_symlink() or not source.is_file():
                raise ValueError(f'{name} must be a regular executable')
            executable = root / (name + suffix)
            shutil.copyfile(source, executable)
            executable.chmod(0o755)
            with executable.open('rb') as stream:
                hashes[executable.name] = hashlib.file_digest(stream, 'sha256').hexdigest()
            result = subprocess.run([str(executable), option], cwd=root, env=env,
                                    stdin=subprocess.DEVNULL, capture_output=True,
                                    text=True, timeout=30)
            if result.returncode:
                raise ValueError(f'{name} build identity failed; rebuild both app entries together: {result.stderr}')
            try:
                record = json.loads(result.stdout)
            except json.JSONDecodeError as error:
                raise ValueError(f'{name} returned invalid build identity') from error
            fields = {'schema', 'version', 'os', 'architecture', 'native_sdk'}
            if not isinstance(record, dict) or set(record) != fields:
                raise ValueError(f'{name} returned an unsupported build identity')
            if type(record['schema']) is not int or record['schema'] != 1:
                raise ValueError(f'{name} returned an unsupported build identity schema')
            if any(not isinstance(record[key], str) or not record[key]
                   for key in fields - {'schema'}):
                raise ValueError(f'{name} returned invalid build identity fields')
            if re.fullmatch('[0-9a-f]{16}', record['native_sdk']) is None:
                raise ValueError(f'{name} returned an invalid native build recipe')
            expected_arch = 'x86' if architecture == 'i686' else architecture
            if ((version is not None and record['version'] != version)
                    or record['os'] != ('macos' if system == 'Darwin' else system.lower())
                    or (expected_arch is not None and record['architecture'] != expected_arch)):
                raise ValueError(f'{name} build identity does not match the requested package')
            records.append(record)
        if records[0] != records[1]:
            raise ValueError('CLI and GUI use different native build recipes; rebuild both app entries together')
        if library.read_bytes() != b'untouched' or config.read_text() != 'deliberately invalid settings\n':
            raise ValueError('Build identity changed player files')
    return {'identity': records[0], 'executables_sha256': hashes}
