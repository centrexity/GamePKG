"""Create the native compatibility source bundle at maintainer build time."""
import argparse
import gzip
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
ROOTS = ['src', 'compat', 'engines', 'include', 'vendor', 'web-player', 'wit', 'html/library']
FILES = [
    'Cargo.toml', 'Cargo.lock', 'build.rs', 'LICENSE', 'NOTICE',
    'html/assets/gamepkg-icon.png',
    'html/support/details.json',
    'examples/maintainer/native_service_runtime.rs',
    'examples/maintainer/service_window.rs',
    'tests/fixtures/service_takedawn_templates.rs', 'tests/support/adversarial.rs',
    'src/port_host/graphics/takedawn.rs', 'src/port_host/graphics/verified.rs',
    'src/port_host/preparation_local.rs', 'src/port_host/process_local.rs',
    'src/port_host/sdk_source.rs',
]
ARTWORK = ['2048', 'arkanoo', 'doom', 'zelda3']
CONFIG = b'''[source.crates-io]
replace-with = "bundled-registry"

[source.bundled-registry]
directory = "registry"
'''


def source_files():
    output = subprocess.check_output(
        ['git', 'ls-files', '-z', '--cached', '--', *ROOTS],
        cwd=PROJECT,
    )
    files = {name.decode() for name in output.split(b'\x00') if name}
    files.update(FILES)
    files.update(f'ports/{port}/{name}' for port in ARTWORK for name in ['thumbnail.png', 'NOTICE'])
    return sorted(files)


def add(archive, name, source, size):
    entry = tarfile.TarInfo(name)
    entry.mode = 0o644
    entry.size = size
    archive.addfile(entry, source)


def create(destination):
    destination = destination.resolve()
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='gamepkg-compat-sdk-') as staging:
        registry = Path(staging) / 'registry'
        result = subprocess.run(
            ['cargo', 'vendor', '--locked', '--offline', '--versioned-dirs', str(registry)],
            cwd=PROJECT,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )
        if result.returncode:
            raise RuntimeError(result.stderr.decode(errors='replace')[-8192:])
        registry_files = [path for path in registry.rglob('*') if not path.is_dir()]
        inputs = {name: PROJECT / name for name in source_files()}
        inputs.update((path.relative_to(staging).as_posix(), path) for path in registry_files)
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix='.native-sdk-') as temporary:
            with gzip.GzipFile(filename='', fileobj=temporary, mode='wb', mtime=0, compresslevel=6) as compressed:
                with tarfile.open(fileobj=compressed, mode='w|', format=tarfile.GNU_FORMAT) as archive:
                    add(archive, '.cargo/config.toml', io.BytesIO(CONFIG), len(CONFIG))
                    for name, path in sorted(inputs.items()):
                        if path.is_symlink() or not path.is_file():
                            raise ValueError(f'Native SDK input must be a regular file: {name}')
                        with path.open('rb') as source:
                            add(archive, name, source, path.stat().st_size)
            temporary.seek(0)
            digest = hashlib.file_digest(temporary, 'sha256').hexdigest()
            temporary.seek(0)
            # Publish only a complete, closed archive. An existing bundle may
            # already be pinned by a build and must never be silently replaced.
            temporary.flush()
            os.link(temporary.name, destination)
    return digest


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('destination', type=Path)
    print(create(parser.parse_args().destination))
