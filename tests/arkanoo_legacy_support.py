"""Build an explicit pre-recorded-host audio fixture from current game source.

Legacy package compatibility keeps its independently hashed SDL 48 kHz clips;
the current playable package uses original-rate host effects and recorded music.
"""
from pathlib import Path
import shutil


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
    use_legacy_audio(destination)
