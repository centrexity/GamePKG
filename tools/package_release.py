"""Bundle locally built Linux or Windows host programs and dependency notices."""
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
import zipfile
from windows_pe_imports import require_standalone_crt

PROJECT = Path(__file__).resolve().parents[1]


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def is_elf(path):
    with path.open('rb') as binary:
        return binary.read(4) == b'\x7fELF'


def executable_architecture(path, windows):
    with path.open('rb') as binary:
        header = binary.read(64)
        if windows:
            if len(header) < 64 or header[:2] != b'MZ':
                raise ValueError('expected a Windows executable')
            binary.seek(int.from_bytes(header[60:64], 'little'))
            pe = binary.read(6)
            if pe[:4] != b'PE\0\0':
                raise ValueError('invalid Windows PE header')
            machine = int.from_bytes(pe[4:6], 'little')
            architectures = {0x14c: 'i686', 0x8664: 'x86_64'}
        else:
            if len(header) < 20 or header[:4] != b'\x7fELF' or header[5] != 1:
                raise ValueError('expected a little-endian Linux ELF executable')
            machine = int.from_bytes(header[18:20], 'little')
            architectures = {3: 'i686', 62: 'x86_64'}
            if (machine == 3 and header[4] != 1) or (machine == 62 and header[4] != 2):
                raise ValueError('ELF class and architecture differ')
        if machine not in architectures:
            raise ValueError('release packaging supports x86 32-bit and 64-bit executables')
        return architectures[machine]


def write_zip(root, destination):
    with zipfile.ZipFile(destination, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(root.rglob('*')):
            if path.is_file():
                info = zipfile.ZipInfo(path.relative_to(root.parent).as_posix(), (1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.create_system = 3
                info.external_attr = (0o100755 if path.suffix == '.exe' else 0o100644) << 16
                with path.open('rb') as source, archive.open(info, 'w') as target:
                    shutil.copyfileobj(source, target)


def dependency_metadata(target):
    metadata = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--offline', '--all-features', '--filter-platform', target, '--format-version', '1'], cwd=PROJECT))
    companion = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--locked', '--offline', '--manifest-path', 'tools/audio-decoder/Cargo.toml',
         '--filter-platform', 'wasm32-unknown-unknown', '--format-version', '1'], cwd=PROJECT))
    # The decoder is embedded Wasm, so its dependency graph is independent of
    # the app's native target and must be included in notices/source accounting.
    metadata['packages'] = list({p['id']: p for p in metadata['packages'] + companion['packages']}.values())
    metadata['workspace_members'] = list(set(metadata['workspace_members'] + companion['workspace_members']))
    metadata['resolve']['nodes'].extend(companion['resolve']['nodes'])
    return metadata


def package(output_directory, binary_directory, *, system=None, target_triple=None):
    system = system or platform.system()
    if system not in ('Linux', 'Windows'):
        raise ValueError('release packaging supports Linux or Windows')
    windows = system == 'Windows'
    suffix = '.exe' if windows else ''
    architecture = executable_architecture(binary_directory / ('gamepkg' + suffix), windows)
    if executable_architecture(binary_directory / ('gamepkg-gui' + suffix), windows) != architecture:
        raise ValueError('CLI and GUI architectures must match')
    host = ('windows-' if windows else 'linux-') + architecture
    target = target_triple or architecture + ('-pc-windows-msvc' if windows else '-unknown-linux-gnu')
    allowed = {architecture + suffix for suffix in (('-pc-windows-msvc', '-pc-windows-gnu') if windows else ('-unknown-linux-gnu',))}
    if target not in allowed:
        raise ValueError('target triple must match the executable platform and architecture')
    version = tomllib.loads((PROJECT / 'Cargo.toml').read_text())['package']['version']
    name = f'gamepkg-{version}-{host}'
    output_directory.mkdir(parents=True, exist_ok=True)
    destination = output_directory / (name + ('.zip' if windows else '.tar.gz'))
    if destination.exists() or destination.is_symlink():
        raise FileExistsError(destination)
    metadata = dependency_metadata(target)
    resolved = {node['id'] for node in metadata['resolve']['nodes']}
    with tempfile.TemporaryDirectory(prefix='gamepkg-distribution-') as directory:
        root = Path(directory) / name
        (root / 'bin').mkdir(parents=True)
        binaries = {}
        for base in ['gamepkg', 'gamepkg-gui']:
            executable = base + ('.exe' if windows else '')
            source = binary_directory / executable
            if not source.is_file() or source.is_symlink():
                raise ValueError(f'{executable} must be a regular executable')
            if windows:
                with source.open('rb') as binary:
                    if binary.read(2) != b'MZ':
                        raise ValueError(f'{executable} must be a Windows executable')
                require_standalone_crt(source)
            elif not is_elf(source):
                raise ValueError(f'{executable} must be an ELF executable')
            shutil.copyfile(source, root / 'bin' / executable)
            (root / 'bin' / executable).chmod(0o755)
            binaries[executable] = sha256(source)
        for filename in ['LICENSE', 'NOTICE', 'Cargo.lock']:
            shutil.copyfile(PROJECT / filename, root / filename)
        (root / 'tools').mkdir()
        for filename in ['install_slang.py', 'install_dxc.py']:
            shutil.copyfile(PROJECT / 'tools' / filename, root / 'tools' / filename)
        shutil.copytree(PROJECT / 'engines', root / 'engines', ignore=shutil.ignore_patterns('__pycache__', 'target'))
        shutil.copytree(PROJECT / 'examples', root / 'examples', ignore=shutil.ignore_patterns('__pycache__', 'target'))
        shutil.copytree(PROJECT / 'tools/audio-decoder', root / 'tools/audio-decoder', ignore=shutil.ignore_patterns('__pycache__', 'target'))
        shutil.copytree(PROJECT / 'ports', root / 'ports', ignore=shutil.ignore_patterns('.git', '__pycache__', 'target'))
        shutil.copyfile(PROJECT / 'docs' / ('INSTALL_WINDOWS.md' if windows else 'INSTALL.md'), root / 'INSTALL.md')
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
        (root / 'BUILD.json').write_text(json.dumps({'version': version, 'platform': host, 'binary_profile': binary_directory.name, 'libc': platform.libc_ver(), 'executables_sha256': binaries, 'cargo_lock_sha256': sha256(PROJECT / 'Cargo.lock')}, indent=2) + '\n')
        with tempfile.NamedTemporaryFile(dir=output_directory, prefix='.release-', delete=False) as temporary:
            temporary_path = Path(temporary.name)
        try:
            if windows:
                write_zip(root, temporary_path)
            else:
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
    parser.add_argument('--system', choices=['Linux', 'Windows'])
    parser.add_argument('--target', dest='target_triple')
    args = parser.parse_args()
    print(package(args.output_directory.resolve(), args.binary_directory.resolve(), system=args.system, target_triple=args.target_triple))
