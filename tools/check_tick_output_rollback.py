"""Maintainer-only native persistence/output rollback check using a private signed fixture."""
from pathlib import Path
import json
import os
import shutil
import subprocess
import tempfile

project = Path(__file__).resolve().parents[1]
cli = Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', project/'target/release'))/'gamepkg'
assert cli.is_file(), 'build the local CLI first'
with tempfile.TemporaryDirectory(prefix='gamepkg-tick-output-') as temporary:
    root = Path(temporary)
    source = root/'source'
    shutil.copytree(project/'examples/minimal', source)
    manifest = source/'manifest.toml'
    text = manifest.read_text().replace('com.gamepkg.minimal', 'com.gamepkg.tick-output-test')
    text = text.replace('audio = false', 'audio = true').replace('save = false', 'save = true')
    text += '\n[[achievements]]\nid="step"\ntitle="Step"\ndescription="Test tick"\n'
    manifest.write_text('homepage="https://example.org/project"\n'+text)
    (source/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,_:u64)->u64{
        gamepkg::window_control(4,0);
        gamepkg::audio_submit_stereo(&[1,-1]);
        gamepkg::draw_rect(0.0,0.0,10.0,10.0,0xffffffff);
        gamepkg::save_write(&[1]);
        gamepkg::achievement_unlock(0);
        1
    }\n''')
    # Fixed synthetic fixture key; no user or release key is read.
    key = root/'test.key'
    key.write_text('71'*32)
    config = root/'settings.toml'
    config.write_text('automatic_update_checks=false\n')
    library = root/'library'
    environment = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(config),
                       GAMEPKG_TICK_OUTPUT_LIBRARY=str(library))
    def run(*arguments):
        result = subprocess.run([str(cli), *map(str, arguments)], env=environment,
                                capture_output=True, text=True, timeout=120)
        assert result.returncode == 0, result.stdout + '\n' + result.stderr
    package = root/'test.gamepkg'
    run('pack', source, '-o', package, '--key', key)
    run('install', package, '--trust-creator')
    run('build', 'com.gamepkg.tick-output-test')
    result = subprocess.run(['cargo', 'test', '--lib', '--no-run', '--message-format=json'],
                            cwd=project, capture_output=True, text=True, timeout=300)
    assert result.returncode == 0, result.stderr
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
    executable = next(item['executable'] for item in artifacts if item.get('reason') == 'compiler-artifact'
                      and item.get('target', {}).get('name') == 'gamepkg' and item.get('executable'))
    result = subprocess.run([executable,
                            'runtime::persistence_output_tests::native_persistence_failures_discard_audio_and_drawing',
                            '--ignored', '--exact', '--nocapture'], env=environment,
                            capture_output=True, text=True, timeout=30)
    assert result.returncode == 0, result.stdout + '\n' + result.stderr
    assert '1 passed' in result.stdout, 'native rollback test did not run'
print('Native signed-package playback: successful output, then history/save failures discard PCM, drawing and homepage requests.')
