"""Maintainer-only packaging of the pinned native runtime and prepared Rust.

No ISO is accepted or read. The resulting kit builds without generation tools.
It remains private until its manifest is pinned in the host and release gates pass.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for part in iter(lambda: f.read(1048576), b''):
            h.update(part)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['source', 'toolkit', 'prepared', 'glyphs', 'output']:
        parser.add_argument('--' + name, required=True, type=Path)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    lock = json.loads((project / 'engines/takedawn/upstream-lock.json').read_text())
    prepared = args.prepared.resolve()
    conversion = json.loads((prepared / 'conversion.json').read_text())
    recovery = conversion.get('recovered_paths')
    functions, units = (24003, 485) if recovery else (24001, 484)
    native_exports = 24030
    if conversion.get('native_mxcsr'):
        from takedawn_native_mxcsr import receipt
        expected_mxcsr = receipt(project, {'recomp_0004_018': ['001DD63F', '001DD652']})
        if conversion['native_mxcsr'] != expected_mxcsr:
            raise RuntimeError('unsupported native MXCSR adaptation')
        if sha(prepared / 'src/gamepkg_native_mxcsr.rs') != expected_mxcsr['implementation_sha256']:
            raise RuntimeError('prepared native MXCSR implementation differs')
        units += 1
        native_exports += 2
    privileged = conversion.get('native_privileged')
    if privileged:
        from takedawn_native_privileged import ADAPTED_UNITS, receipt as privileged_receipt
        expected_privileged = privileged_receipt(project, ADAPTED_UNITS)
        if privileged != expected_privileged:
            raise RuntimeError('unsupported native cache/interrupt adaptation')
        if sha(prepared / 'src/gamepkg_native_privileged.rs') != privileged['implementation_sha256']:
            raise RuntimeError('prepared native cache/interrupt implementation differs')
        units += 1
        native_exports += len(privileged['native_exports'])
    if recovery:
        spec = project / 'engines/takedawn/rust-port/recovered-paths.json'
        if (recovery['specification_sha256'] != sha(spec)
                or [f['start'] for f in recovery['functions']] != ['0x000966F0', '0x00249AB6']
                or recovery['executable_sha256'] != lock['disc_manifest']['executable_sha256']
                or recovery['original_media_extracted_to_disk'] is not False):
            raise RuntimeError('unsupported recovered native paths')
    if (conversion['game_commit'] != lock['game']['commit']
            or conversion['toolkit_commit'] != lock['toolkit']['commit']
            or conversion['generated_game_functions'] != functions
            or conversion['translation_units'] != units
            or len(conversion['exports']) != native_exports):
        raise RuntimeError('unsupported prepared game inventory')
    for name, digest in conversion['rust_sources'].items():
        if Path(name).name != name or sha(prepared / 'src' / name) != digest:
            raise RuntimeError('prepared Rust hash differs: ' + name)
    for name in ['build.rs', 'units.txt', 'native_tls.c', 'Cargo.toml']:
        if sha(prepared / name) != conversion['prepared_files'][name]:
            raise RuntimeError('prepared build input differs: ' + name)
    output = args.output.absolute()
    parent = output.parent.resolve(strict=True)
    if (not parent.is_relative_to(Path(tempfile.gettempdir()).resolve())
            or parent.stat().st_mode & 0o077 or output.exists() or output.is_symlink()):
        raise RuntimeError('choose a new output under a private temporary directory')
    output.mkdir(mode=0o700)
    files = {}

    def copy(source, name):
        if source.is_symlink() or not source.is_file():
            raise RuntimeError('SDK input must be a regular file: ' + str(source))
        if source.stat().st_size > 64 * 1048576:
            raise RuntimeError('SDK file exceeds 64 MiB: ' + name)
        path = Path(name)
        if path.is_absolute() or any(p in ['', '.', '..'] for p in path.parts) or '\\' in name:
            raise RuntimeError('unsafe SDK name: ' + name)
        if name in files:
            raise RuntimeError('duplicate SDK name: ' + name)
        target = output / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        mode = 0o755 if source.stat().st_mode & 0o111 else 0o644
        os.chmod(target, mode)
        files[name] = {'sha256': sha(target), 'bytes': target.stat().st_size, 'mode': mode}

    for path, key, prefix in [(args.source.resolve(), 'game', 'game'),
                              (args.toolkit.resolve(), 'toolkit', 'toolkit')]:
        revision = subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True).strip()
        if revision != lock[key]['commit']:
            raise RuntimeError('unreviewed ' + key + ' revision')
        subprocess.run(['git', '-C', str(path), 'diff', '--quiet', 'HEAD', '--'], check=True)
        names = subprocess.check_output(['git', '-C', str(path), 'ls-files', '-z']).decode().split('\0')
        for name in filter(None, names):
            if key == 'toolkit' and name == 'tools/conformance/test.xbe':
                continue  # Synthetic maintainer-only CPU fixture, never runtime input.
            if Path(name).suffix.lower() in ['.iso', '.xbe', '.xmv', '.xwb', '.xsb']:
                raise RuntimeError('disc-like input in runtime source: ' + name)
            copy(path / name, prefix + '/' + name)
    if privileged:
        from takedawn_native_privileged import patch_kernel
        implementation = project / 'engines/takedawn/rust-port/native_irq.inc.h'
        copy(implementation, 'host/engines/takedawn/rust-port/native_irq.inc.h')
        name = 'toolkit/src/kernel/kernel_hal.c'
        patched = output / name
        patched.write_text(patch_kernel(patched.read_text(), implementation.read_text()))
        files[name] = {'sha256': sha(patched), 'bytes': patched.stat().st_size, 'mode': 0o644}
    # Only generated declarations/stamp are needed by the original runtime.
    # No generated C game bodies are shipped or compiled.
    gen = args.source / 'src/game/recomp/gen'
    for name in ['recomp_funcs.h', 'recomp_types.h', 'regen.stamp']:
        if (gen / name).is_file():
            copy(gen / name, 'game/src/game/recomp/gen/' + name)
    for name in ['bo3_glyph_art.c', 'bo3_glyph_art.h']:
        copy(args.glyphs / name, 'glyphs/' + name)
    for name in conversion['rust_sources']:
        copy(prepared / 'src' / name, 'prepared/src/' + name)
    for name in ['build.rs', 'Cargo.toml', 'units.txt', 'native_tls.c', 'conversion.json']:
        copy(prepared / name, 'prepared/' + name)
    export_path = output / 'prepared/exports.txt'
    export_path.write_text('\n'.join(conversion['exports']) + '\n')
    os.chmod(export_path, 0o644)
    files['prepared/exports.txt'] = {'sha256': sha(export_path), 'bytes': export_path.stat().st_size, 'mode': 0o644}
    host = project / 'engines/takedawn'
    for name in ['Cargo.toml', 'Cargo.lock', 'src/lib.rs', 'src/windows_handle.rs', 'media.h', 'posix.c',
                 'engine.c', 'attach.cmake', 'adapt_paths.cmake', 'adapt_smoke.cmake', 'adapt_memory.cmake',
                 'adapt_input_glyphs.cmake', 'input_glyphs_policy.h',
                 'windows_file.c', 'windows_file.h', 'windows_stream.c', 'windows_stream.h',
                 'windows_entry.c', 'windows_attach.cmake', 'windows_restart.h', 'adapt_windows_restart.cmake', 'adapt_windows_file.cmake',
                 'adapt_windows_startup.cmake', 'DotNET-RUNTIME-LICENSE.txt']:
        copy(host / 'media-bridge' / name, 'host/engines/takedawn/media-bridge/' + name)
    for name in ['attach.cmake', 'C2Rust-LICENSE.txt', 'rustc-cache.rs', 'rustc-cache-linux-v1.rs', 'build-windows-units.rs', 'build-windows-runtime.rs']:
        copy(host / 'rust-port' / name, 'host/engines/takedawn/rust-port/' + name)
    for name in ['ntsc-u-retail.manifest', 'upstream-lock.json', 'Takedawn-LICENSE.txt',
                 'Xboxrecomp-LICENSE.txt', 'windows-dependencies.json']:
        copy(host / name, 'host/engines/takedawn/' + name)
    for name in ['memory_budget.rs', 'xbox_disc.rs', 'xbox_disc/media.rs']:
        copy(project / 'src' / name, 'host/src/' + name)
    # Vendor the already locked, reviewed host-media dependencies. Generated
    # game code itself has no third-party Rust dependencies.
    with tempfile.TemporaryDirectory(prefix='gamepkg-sdk-vendor-', dir=parent) as temporary:
        vendor = Path(temporary) / 'vendor'
        subprocess.run(['cargo', 'vendor', '--offline', '--locked', '--versioned-dirs',
                        '--manifest-path', str(host / 'media-bridge/Cargo.toml'), str(vendor)],
                       check=True, stdout=subprocess.DEVNULL)
        for item in sorted(vendor.rglob('*')):
            if item.is_file() or item.is_symlink():
                copy(item, 'host/vendor/' + item.relative_to(vendor).as_posix())
        config = Path(temporary) / 'config.toml'
        config.write_text('[source.crates-io]\nreplace-with="gamepkg-vendored"\n'
                          '[source.gamepkg-vendored]\ndirectory="../../../vendor"\n')
        copy(config, 'host/engines/takedawn/media-bridge/.cargo/config.toml')
    manifest = {'format': 'gamepkg-takedawn-source-kit-v1', 'game_commit': lock['game']['commit'],
                'toolkit_commit': lock['toolkit']['commit'], 'generated_game_functions': functions,
                'native_exports': native_exports, 'files': files,
                'omitted_maintainer_fixtures': ['toolkit/tools/conformance/test.xbe']}
    data = json.dumps(manifest, sort_keys=True, indent=2).encode()
    (output / 'kit.json').write_bytes(data)
    archive = output / 'source-kit.tar.gz'
    with archive.open('xb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0, compresslevel=6) as gz:
        with tarfile.open(fileobj=gz, mode='w|') as tar:
            # Manifest first permits bounded verification while streaming.
            for name in ['kit.json'] + sorted(files):
                payload = output / name
                info = tarfile.TarInfo(name)
                info.size = payload.stat().st_size
                info.mode = files[name]['mode'] if name != 'kit.json' else 0o644
                info.mtime = 0
                with payload.open('rb') as content:
                    tar.addfile(info, content)
    receipt = {'sha256': sha(archive), 'archive_bytes': archive.stat().st_size,
               'unpacked_bytes': sum(f['bytes'] for f in files.values()), 'file_count': len(files)}
    (output / 'receipt.json').write_text(json.dumps(receipt, indent=2))
    print(json.dumps(receipt), flush=True)
    print('Private source kit prepared; offline native build, isolation and app integration still require qualification.')


if __name__ == '__main__':
    main()
