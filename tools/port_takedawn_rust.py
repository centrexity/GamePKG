"""Maintainer-only: port freshly generated, pinned Takedawn C to Rust.

Python, Clang and C2Rust are release-preparation tools, not player requirements.
The selected output must be a new private /tmp directory. No ISO assets or XBE
are copied here. This command never publishes its game-derived output.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--toolkit', type=Path, required=True)
parser.add_argument('--c2rust', type=Path, required=True)
parser.add_argument('--normalizer', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--clang', default='/usr/bin/clang-18')
parser.add_argument('--jobs', type=int, default=2, choices=range(1, 5))
parser.add_argument('--structured-unit', action='append', default=['recomp_0021_013'],
                    help='Use C2Rust structured lowering for a reviewed hot unit (repeatable)')
parser.add_argument('--reuse', type=Path, help='Reuse matching successful raw conversions from a private earlier attempt')
parser.add_argument('--recover-gameplay-paths', type=Path, metavar='ISO',
                    help='Recover reviewed missing native paths from the supported ISO in RAM')
parser.add_argument('--native-mxcsr', action='store_true',
                    help='Restore the reviewed native SSE setup pair using real x86 control state')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
lock = json.loads((project / 'engines/takedawn/upstream-lock.json').read_text())
source, toolkit = args.source.resolve(), args.toolkit.resolve()
for path, key in [(source, 'game'), (toolkit, 'toolkit')]:
    revision = subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != lock[key]['commit']:
        raise RuntimeError(f'{key}: source revision differs from the lock')
    subprocess.run(['git', '-C', str(path), 'diff', '--quiet', 'HEAD', '--'], check=True)
gen = source / 'src/game/recomp/gen'
stamp = dict(line.split(' ', 1) for line in (gen / 'regen.stamp').read_text().splitlines())
if stamp['patches'] != '108/108' or stamp['spin_waits'] != '2' or stamp['files'] != '29':
    raise RuntimeError('the complete pinned C generation is required')

output = args.output.absolute()
parent = output.parent.resolve(strict=True)
temporary = Path(tempfile.gettempdir()).resolve()
if not parent.is_relative_to(temporary) or parent == temporary or parent.stat().st_mode & 0o077:
    raise RuntimeError('output parent must be a private temporary subdirectory')
if output.exists() or output.is_symlink():
    raise RuntimeError('refusing to overwrite an earlier conversion')
output = parent / output.name
output.mkdir(mode=0o700)
split, rust, reports, logs = [output / part for part in ['c', 'src', 'reports', 'logs']]
for folder in [split, rust, reports, logs]:
    folder.mkdir(mode=0o700)

# Generated chunks contain only void(void) game function definitions after the
# common preamble. Split at their top-level starts, retaining every byte of each
# body. Smaller translation units avoid C2Rust's large-unit AST exporter crash.
pattern = re.compile(r'^void (sub_[A-Za-z0-9_]+|xbe_entry_point)\(void\)\n\{', re.MULTILINE)
units, expected, inputs = [], set(), {}
for original in sorted(gen.glob('recomp_*.c')):
    text = original.read_text()
    inputs[original.name] = hashlib.sha256(original.read_bytes()).hexdigest()
    matches = list(pattern.finditer(text))
    if re.fullmatch(r'recomp_\d{4}\.c', original.name):
        if not matches:
            raise RuntimeError(f'no game definitions in {original.name}')
        prefix = text[:matches[0].start()]
        for i in range(0, len(matches), 50):
            names = [m.group(1) for m in matches[i:i + 50]]
            if expected.intersection(names) or len(set(names)) != len(names):
                raise RuntimeError('duplicate generated game function')
            expected.update(names)
            end = matches[i + 50].start() if i + 50 < len(matches) else len(text)
            unit = split / f'{original.stem}_{i // 50:03}.c'
            unit.write_text(prefix + text[matches[i].start():end])
            units.append((unit, names))
    else:
        unit = split / original.name
        unit.write_text(text)
        units.append((unit, []))
if len(expected) != 24001:
    raise RuntimeError(f'expected 24001 generated game functions, found {len(expected)}')
recovery = None
if args.recover_gameplay_paths:
    from takedawn_recovered_paths import recover, remove_recovered_stubs
    code, recovery = recover(project, source, toolkit, args.recover_gameplay_paths)
    unit = split / 'recomp_recovered.c'
    unit.write_text(code)
    names = [f"sub_{int(entry['start'], 16):08X}" for entry in recovery['functions']]
    if expected.intersection(names):
        raise RuntimeError('recovered native functions already exist')
    expected.update(names)
    units.append((unit, names))
    stubs = split / 'recomp_stubs_unresolved.c'
    stubs.write_text(remove_recovered_stubs(stubs.read_text(), recovery['functions']))
manual = source / 'src/game/recomp/recomp_manual.c'
inputs['recomp_manual.c'] = hashlib.sha256(manual.read_bytes()).hexdigest()
unit = split / manual.name
unit.write_bytes(manual.read_bytes())
units.append((unit, []))

mxcsr = None
if args.native_mxcsr:
    from takedawn_native_mxcsr import adapt, receipt
    adapted = {}
    for unit, _ in units:
        text, seen = adapt(unit.read_text())
        if seen:
            unit.write_text(text)
            adapted[unit.stem] = seen
    mxcsr = receipt(project, adapted)

unknown_structured = set(args.structured_unit) - {unit.stem for unit, _ in units}
if unknown_structured:
    raise RuntimeError(f'unknown structured units: {sorted(unknown_structured)}')

commands = []
for unit, _ in units:
    commands.append({'directory': str(split), 'file': str(unit), 'arguments': [
        args.clang, '-std=gnu11', '-Wno-parentheses-equality', '-fwrapv',
        '-fno-strict-aliasing', '-I' + str(gen), '-I' + str(manual.parent),
        '-I' + str(source / 'src/game'), '-I' + str(toolkit / 'templates/runtime'),
        '-I' + str(toolkit / 'src'), '-I' + str(toolkit / 'src/kernel'), '-c', str(unit)]})
database = split / 'compile_commands.json'
database.write_text(json.dumps(commands))

def convert(entry):
    unit, names = entry
    structured = unit.stem in args.structured_unit
    strategy = 'structured' if structured else 'bounded-flat-large-cfg'
    # Each invocation has a separate raw directory and log. Nothing is retried
    # with a permissive mode when a definition cannot be translated.
    raw = output / ('raw-' + unit.stem)
    with (logs / (unit.stem + '.log')).open('w') as log:
        reused = False
        if args.reuse:
            previous = args.reuse.resolve()
            old_c = previous / 'c' / unit.name
            old_rust = previous / ('raw-' + unit.stem) / 'src' / (unit.stem + '.rs')
            old_report = previous / 'reports' / (unit.stem + '.json')
            if (old_c.is_file() and old_rust.is_file() and old_report.is_file()
                    and old_c.read_bytes() == unit.read_bytes()
                    and json.loads(old_report.read_text()).get('control_flow_preparation',
                        'bounded-flat-large-cfg') == strategy):
                (raw / 'src').mkdir(parents=True)
                (raw / 'src' / old_rust.name).write_bytes(old_rust.read_bytes())
                reused = True
        if not reused:
            command = [str(args.c2rust.resolve()), 'transpile', '--fail-on-error',
                       '--disable-rustfmt', '--emit-modules', '--filter', unit.name,
                       '--output-dir', str(raw), str(database)]
            environment = dict(os.environ)
            environment.pop('GAMEPKG_C2RUST_FLAT_CFG', None)
            if not structured:
                command[3:3] = ['--no-incremental-relooper', '--no-simplify-structures']
                environment['GAMEPKG_C2RUST_FLAT_CFG'] = '1'
            child = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                     start_new_session=True, env=environment)
            try:
                status = child.wait(timeout=90)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()
                raise RuntimeError(f'{unit.name}: conversion exceeded 90 seconds; no function was substituted')
            if status:
                raise subprocess.CalledProcessError(status, command)
        report = reports / (unit.stem + '.json')
        subprocess.run([str(args.normalizer.resolve()), str(raw / 'src' / (unit.stem + '.rs')),
                        str(rust / (unit.stem + '.rs')), str(report)],
                       stdout=log, stderr=subprocess.STDOUT, check=True)
    metadata = json.loads(report.read_text())
    metadata['control_flow_preparation'] = strategy
    report.write_text(json.dumps(metadata, indent=2))
    if not set(names).issubset(metadata['exports']):
        raise RuntimeError(f'{unit.name}: generated functions were lost during translation')
    print(f'Converted {unit.stem}: {len(names)} original game functions', flush=True)
    return metadata

tls, exports, strategies = {}, set(), {}
with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
    for (unit, _), metadata in zip(units, pool.map(convert, units)):
        strategies[unit.stem] = metadata['control_flow_preparation']
        for name, ty in metadata['thread_locals'].items():
            if name in tls and tls[name] != ty:
                raise RuntimeError(f'conflicting TLS layout for {name}')
            tls[name] = ty
        for name in metadata['exports']:
            if name in exports:
                raise RuntimeError(f'duplicate native symbol: {name}')
            exports.add(name)
if not expected.issubset(exports):
    raise RuntimeError('converted export set is incomplete')
prepared_units = [unit.stem for unit, _ in units]
if mxcsr:
    # A reviewed leaf Rust unit owns the actual native control instructions.
    # Generated game bodies remain Rust; no C shim implements game behavior.
    name = 'gamepkg_native_mxcsr'
    if exports.intersection(mxcsr['native_exports']):
        raise RuntimeError('duplicate native MXCSR support symbol')
    (rust / (name + '.rs')).write_bytes(
        (project / 'engines/takedawn/rust-port/native_mxcsr.rs').read_bytes())
    prepared_units.append(name)
    exports.update(mxcsr['native_exports'])
shim = '#define RECOMP_GENERATED_CODE\n#include "recomp_types.h"\n'
for name in sorted(tls):
    if not re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*', name):
        raise RuntimeError('invalid TLS symbol')
    shim += f'void *gamepkg_tls_{name}(void) {{ return (void *)&{name}; }}\n'
(output / 'native_tls.c').write_text(shim)
# Compile independent prepared units with a stable Rust build script. A single
# enormous module tree multiplies compiler memory and cannot reuse unit objects.
(output / 'units.txt').write_text(''.join(name + '\n' for name in prepared_units))
(rust / 'lib.rs').write_text('// Prepared game units are bundled by build.rs.\n')
(output / 'build.rs').write_bytes((project / 'engines/takedawn/rust-port/prepared-build.rs').read_bytes())
(output / 'Cargo.toml').write_text('''[package]
name = "gamepkg-takedawn-generated"
version = "0.1.0"
edition = "2021"
publish = false
[workspace]
[lib]
crate-type = ["staticlib"]
[profile.release]
opt-level = 3
overflow-checks = false
panic = "abort"
''')
manifest = {'game_commit': lock['game']['commit'], 'toolkit_commit': lock['toolkit']['commit'],
            'c_stamp': stamp, 'c_sources': inputs, 'generated_game_functions': len(expected),
            'translation_units': len(prepared_units), 'control_flow_preparation': strategies, 'exports': sorted(exports), 'thread_locals': tls,
            'rust_sources': {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                             for p in sorted(rust.glob('*.rs'))}}
manifest['prepared_files'] = {name: hashlib.sha256((output / name).read_bytes()).hexdigest()
                              for name in ['build.rs', 'units.txt', 'native_tls.c', 'Cargo.toml']}
manifest['preparation'] = {
    'c2rust_sha256': hashlib.sha256(args.c2rust.read_bytes()).hexdigest(),
    'normalizer_sha256': hashlib.sha256(args.normalizer.read_bytes()).hexdigest(),
    'converter_patch_sha256': hashlib.sha256((project / 'engines/takedawn/rust-port/c2rust-relooper.patch').read_bytes()).hexdigest(),
    'prepared_build_sha256': hashlib.sha256((output / 'build.rs').read_bytes()).hexdigest(),
    'units_sha256': hashlib.sha256((output / 'units.txt').read_bytes()).hexdigest(),
    'large_control_flow': 'native Rust branches over the original typed basic blocks',
}
if recovery:
    manifest['recovered_paths'] = recovery
if mxcsr:
    manifest['native_mxcsr'] = mxcsr
(output / 'conversion.json').write_text(json.dumps(manifest, indent=2))
print(f'Prepared all {len(expected)} game functions in {len(prepared_units)} Rust units.', flush=True)
print('Compilation, C/Rust behavior comparison and gameplay validation remain required.', flush=True)
