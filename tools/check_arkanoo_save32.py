"""Execute current archive regressions in a real 32-bit process.

Maintainer-only: requires the i686 Rust target, linker and 32-bit C runtime.
"""
from pathlib import Path
import argparse, os, re, subprocess, tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rustc', default='rustc')
parser.add_argument('--linker', default='cc', type=Path)
parser.add_argument('--runtime-libraries', type=Path)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
source = (project/'tests/game_arkanoo.rs').read_text()
names = [
    'editor_archive_migrates_legacy_and_rejects_damage_without_partial_replacement',
    'legacy_gravity_flags_reject_corruption_without_changing_existing_patterns_or_audio',
    'effects_archive_migrates_old_formats_preserves_capacity_and_rejects_bad_settings',
]
sections = {match.group(1): match.group(0) for match in re.finditer(
    r'^#\[test\]\nfn (\w+)\([^\n]*\).*?(?=^#\[test\]|\Z)', source, re.M | re.S)}
assert all(name in sections for name in names), 'Current archive tests were not found'
with tempfile.TemporaryDirectory(prefix='gamepkg-save32-') as temporary:
    root = Path(temporary)
    harness = root/'save32.rs'
    harness.write_text('#[cfg(not(target_pointer_width="32"))]\ncompile_error!("Requires a real 32-bit target");\n'
        + f'#[path="{project}/ports/arkanoo/source/src/editor.rs"]\nmod editor;\n'
        + '\n'.join(sections[name] for name in names))
    binary = root/'save32'
    subprocess.run([args.rustc, '--edition=2021', '--test', '-O', '--target',
        'i686-unknown-linux-gnu', '-C', 'linker='+str(args.linker), '-A', 'dead_code',
        str(harness), '-o', str(binary)], check=True, timeout=120)
    assert binary.read_bytes()[:5] == b'\x7fELF\x01', 'Archive tests must be ELF32'
    environment = dict(os.environ)
    if args.runtime_libraries:
        environment['LD_LIBRARY_PATH'] = str(args.runtime_libraries.resolve())
    subprocess.run([str(binary), '--nocapture'], check=True, env=environment, timeout=120)
print('Current legacy, corruption, audio-settings and maximum-archive tests pass as ELF32.')
