"""Build an explicit pre-recorded-host audio fixture from current game source.

Legacy package compatibility keeps its independently hashed SDL 48 kHz clips;
the current playable package uses original-rate host effects and recorded music.
"""
from pathlib import Path
import hashlib
import json
import shutil
import zipfile
import tomllib


def recorded_memory_budget(port: Path) -> tuple[bool, int]:
    manifest = tomllib.loads((port / 'manifest.toml').read_text())
    limits = manifest['limits']
    profile = limits.get('render_profile')
    assert profile in (None, 'large-images-v1')
    large = profile == 'large-images-v1'
    wasm_hard = 268435456 if large else 16777216
    if manifest.get('native_tick'):
        assert large and limits['hard_memory_bytes'] == 536870912
    else:
        assert limits['hard_memory_bytes'] == wasm_hard
    assert (limits['effects_memory_bytes'], limits['music_memory_bytes']) == (3145728, 7340032)
    return large, wasm_hard


def wasm_fixture_manifest(text: str) -> str:
    """Convert only disposable Wasm test recipes, not the signed native port."""
    if not tomllib.loads(text).get('native_tick'):
        return text
    lines = []
    for line in text.splitlines():
        key, _, value = line.partition('=')
        if key.strip() == 'native_tick':
            continue
        if key.strip() == 'hard_memory_bytes' and value.strip() == '536870912':
            line = 'hard_memory_bytes = 268435456'
        lines.append(line)
    return '\n'.join(lines) + '\n'


def fixture_module_declarations(text: str) -> str:
    """Keep the port's modules without duplicating its singleton or tick."""
    modules = [line for line in text.splitlines() if line.startswith('mod ') and line.endswith(';')]
    assert 'mod physics;' in modules and 'mod render;' in modules
    return '\n'.join(modules) + '\n'


def use_legacy_audio(recipe: Path) -> None:
    project = Path(__file__).resolve().parents[1]
    manifest = recipe / 'manifest.toml'
    lines = manifest.read_text().splitlines()
    lines = [line for line in lines if not line.startswith(('effects_memory_bytes', 'music_memory_bytes'))]
    manifest.write_text('\n'.join(lines).replace('recommended_memory_bytes = 16777216', 'recommended_memory_bytes = 8388608') + '\n')
    (recipe / 'assets/audio/music-list.txt').unlink(missing_ok=True)
    shutil.rmtree(recipe / 'assets/music', ignore_errors=True)
    for recording in (project / 'tests/fixtures/arkanoo-legacy-effects').glob('*.wav'):
        shutil.copy2(recording, recipe / 'assets/audio' / recording.name)


def copy_legacy_port(destination: Path) -> None:
    project = Path(__file__).resolve().parents[1]
    shutil.copytree(project / 'ports/arkanoo', destination)
    # Freeze only the historical drawing path, derived artwork and attribution.
    # Physics, input, saves and other source remain the current implementation.
    # This keeps the original byte-exact references and 8 MiB frame limit useful
    # when the actual port selects source-resolution artwork and larger buffers.
    fixture = project / 'tests/fixtures/arkanoo-legacy-artwork.zip'
    metadata = json.loads(fixture.with_suffix('.json').read_text())
    assert hashlib.sha256(fixture.read_bytes()).hexdigest() == metadata['archive_sha256']
    expected = {row['path']: row for row in metadata['files']}
    assert len(expected) == 47
    for pattern in ['background*.png', 'menu-background*.png']:
        for path in (destination / 'assets').glob(pattern):
            path.unlink()
    with zipfile.ZipFile(fixture) as archive:
        assert set(archive.namelist()) == set(expected) and len(archive.infolist()) == 47
        for info in archive.infolist():
            name = Path(info.filename)
            assert not name.is_absolute() and '..' not in name.parts and '\\' not in info.filename
            row = expected[info.filename]
            assert info.file_size == row['bytes'] and info.file_size <= 2 * 1048576
            data = archive.read(info)
            assert hashlib.sha256(data).hexdigest() == row['sha256']
            target = destination / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(data)
    manifest = destination / 'manifest.toml'
    lines = [line for line in wasm_fixture_manifest(manifest.read_text()).splitlines()
             if not line.startswith('render_profile')]
    lines = ['hard_memory_bytes = 16777216' if line.startswith('hard_memory_bytes') else
             'recommended_memory_bytes = 8388608' if line.startswith('recommended_memory_bytes') else line
             for line in lines]
    manifest.write_text('\n'.join(lines) + '\n')
    use_legacy_audio(destination)
