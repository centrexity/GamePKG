"""Bundle locally built Linux host programs, setup tools, examples, and license notices."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

PROJECT = Path(__file__).resolve().parents[1]


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def is_elf(path):
    with path.open('rb') as binary:
        return binary.read(4) == b'\x7fELF'


def package(output_directory, binary_directory):
    if platform.system() != 'Linux' or platform.machine() not in ('x86_64', 'amd64'):
        raise ValueError('release packaging currently supports Linux x86-64')
    version = tomllib.loads((PROJECT / 'Cargo.toml').read_text())['package']['version']
    name = f'gamepkg-{version}-linux-x86_64'
    output_directory.mkdir(parents=True, exist_ok=True)
    destination = output_directory / f'{name}.tar.gz'
    if destination.exists() or destination.is_symlink():
        raise FileExistsError(destination)
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--offline', '--all-features', '--filter-platform', 'x86_64-unknown-linux-gnu', '--format-version', '1'], cwd=PROJECT))
    resolved = {node['id'] for node in metadata['resolve']['nodes']}
    with tempfile.TemporaryDirectory(prefix='gamepkg-distribution-') as directory:
        root = Path(directory) / name
        (root / 'bin').mkdir(parents=True)
        binaries = {}
        for executable in ['gamepkg', 'gamepkg-gui']:
            source = binary_directory / executable
            if not source.is_file() or source.is_symlink() or not is_elf(source):
                raise ValueError(f'{executable} must be a regular ELF release executable')
            shutil.copyfile(source, root / 'bin' / executable)
            (root / 'bin' / executable).chmod(0o755)
            binaries[executable] = sha256(source)
        for filename in ['LICENSE', 'NOTICE', 'Cargo.lock']:
            shutil.copyfile(PROJECT / filename, root / filename)
        (root / 'tools').mkdir()
        for filename in ['install_slang.py', 'install_dxc.py']:
            shutil.copyfile(PROJECT / 'tools' / filename, root / 'tools' / filename)
        shutil.copytree(PROJECT / 'examples', root / 'examples', ignore=shutil.ignore_patterns('__pycache__', 'target'))
        shutil.copyfile(PROJECT / 'docs' / 'INSTALL.md', root / 'INSTALL.md')
        notices = []
        for dependency in sorted(metadata['packages'], key=lambda p: p['id']):
            if dependency['id'] in metadata['workspace_members'] or dependency['id'] not in resolved:
                continue
            source = Path(dependency['manifest_path']).parent
            shutil.copytree(source, root / 'third-party-sources' / f"{dependency['name']}-{dependency['version']}")
            documents = {p for p in source.iterdir() if p.is_file() and p.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'COPYRIGHT'))}
            if dependency.get('license_file'):
                documents.add(source / dependency['license_file'])
            if (source / 'LICENSES').is_dir():
                documents.update(p for p in (source / 'LICENSES').rglob('*') if p.is_file())
            target = root / 'third-party-licenses' / f"{dependency['name']}-{dependency['version']}"
            for document in sorted(documents):
                relative = document.relative_to(source)
                (target / relative).parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(document, target / relative)
            notices.append({'name': dependency['name'], 'version': dependency['version'], 'license': dependency.get('license'), 'source': dependency.get('source'), 'notices': [str(p.relative_to(source)) for p in sorted(documents)]})
        (root / 'DEPENDENCIES.json').write_text(json.dumps(notices, indent=2) + '\n')
        (root / 'BUILD.json').write_text(json.dumps({'version': version, 'platform': 'linux-x86_64', 'libc': platform.libc_ver(), 'executables_sha256': binaries, 'cargo_lock_sha256': sha256(PROJECT / 'Cargo.lock')}, indent=2) + '\n')
        with tempfile.NamedTemporaryFile(dir=output_directory, prefix='.release-', delete=False) as temporary:
            temporary_path = Path(temporary.name)
        try:
            with temporary_path.open('wb') as raw, gzip.GzipFile(filename='', fileobj=raw, mode='wb', mtime=0) as compressed, tarfile.open(fileobj=compressed, mode='w') as archive:
                for path in [root, *sorted(root.rglob('*'))]:
                    info = archive.gettarinfo(str(path), arcname=str(path.relative_to(root.parent)))
                    info.uid = info.gid = 0
                    info.uname = info.gname = ''
                    info.mtime = 0
                    if path.is_file():
                        with path.open('rb') as data:
                            archive.addfile(info, data)
                    else:
                        archive.addfile(info)
            os.link(temporary_path, destination)
        finally:
            temporary_path.unlink(missing_ok=True)
    checksum = f'{sha256(destination)}  {destination.name}\n'
    checksum_path = destination.with_suffix(destination.suffix + '.sha256')
    with checksum_path.open('x') as output:
        output.write(checksum)
    return destination


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output_directory', type=Path)
    parser.add_argument('--binary-directory', type=Path, default=PROJECT / 'target' / 'release')
    args = parser.parse_args()
    print(package(args.output_directory.resolve(), args.binary_directory.resolve()))
