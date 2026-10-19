"""Maintainer-only signed no-dependency effect SDK check. No player prerequisites."""
from pathlib import Path
import json, os, shutil, subprocess, tempfile, wave
project = Path(__file__).resolve().parents[1]
cli = project/'target/release/gamepkg'
compiled = subprocess.run(['cargo', 'test', '--locked', '--release', '--features', 'audio', '--test', 'effects_sdk', '--no-run', '--message-format=json'], cwd=project, capture_output=True, text=True, timeout=300)
assert compiled.returncode == 0, compiled.stderr
artifacts = [json.loads(line) for line in compiled.stdout.splitlines() if line.startswith('{')]
executables = [item['executable'] for item in artifacts if item.get('reason') == 'compiler-artifact' and item.get('target', {}).get('name') == 'effects_sdk' and item.get('executable')]
assert len(executables) == 1
with tempfile.TemporaryDirectory(prefix='gamepkg-effects-sdk-') as temporary:
    root = Path(temporary)
    environment = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'), GAMEPKG_EFFECT_SDK_LIBRARY=str(root/'library'))
    (root/'null-alsa.conf').write_text('pcm.!default { type null }\n')
    environment['ALSA_CONFIG_PATH'] = str(root/'null-alsa.conf')
    (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    key = root/'synthetic.key'; key.write_text('68'*32)
    def run(*arguments, success=True):
        result = subprocess.run([str(cli), *map(str, arguments)], env=environment, capture_output=True, text=True, timeout=180)
        assert (result.returncode == 0) == success, result.stdout+'\n'+result.stderr
        return result
    for profile, suffix in [('core-v1', 'core'), ('component-v1', 'component')]:
        for enabled in [False, True]:
            id = f'com.gamepkg.effects-{suffix}-'+('on' if enabled else 'off')
            recipe = root/id; shutil.copytree(project/'examples/minimal', recipe)
            path = recipe/'manifest.toml'
            text = path.read_text().replace('com.gamepkg.minimal', id).replace('audio = false', 'audio = true').replace('save = false', 'save = true')
            if enabled: text = text.replace('[limits]', '[limits]\neffects_memory_bytes=1048576')
            text += '\n[[achievements]]\nid="first"\ntitle="First"\ndescription="Fixture"\n[[achievements]]\nid="second"\ntitle="Second"\ndescription="Fixture"\n[[achievements]]\nid="third"\ntitle="Third"\ndescription="Fixture"\n'
            path.write_text('runtime_profile="'+profile+'"\n'+text)
            with wave.open(str(recipe/'assets/effect.wav'), 'wb') as output:
                output.setnchannels(2); output.setsampwidth(2); output.setframerate(44100)
                output.writeframes((int(12345).to_bytes(2,'little',signed=True)+int(-23456).to_bytes(2,'little',signed=True))*4096)
            (recipe/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,_:u64)->u64 {
                let mut result=0u64;
                if gamepkg::effect_volume(64) {result|=1;}
                if gamepkg::effect_pause(false) {result|=2;}
                if gamepkg::effect_play("effect.wav") {result|=4;}
                if gamepkg::effect_stop() {result|=8;}
                if gamepkg::effect_play("effect.wav") {result|=16;}
                if !gamepkg::effect_volume(129) {result|=32;}
                if !gamepkg::effect_play("../effect.wav") {result|=64;}
                if !gamepkg::audio_submit(&[]) {result|=128;}
                if gamepkg::save_write(&[1]) {result|=256;}
                if gamepkg::achievement_unlock(input) {result|=512;}
                let divisor:u32=if input==1 {0}else{1}; result+u64::from(10/divisor)
            }''')
            archive = root/(id+'.gamepkg')
            run('pack', recipe, '-o', archive, '--key', key)
            run('install', archive, '--trust-creator')
            run('build', id, '--target', 'wasm')
            if enabled:
                result = run('web-export', id, '--output', root/(id+'-web'), success=False)
                assert 'recorded-effect output is not yet available' in result.stderr
    subprocess.run([executables[0], '--ignored', '--nocapture'], cwd=project, env=environment, check=True, timeout=120)
    for enabled in [False, True]:
        run('build', 'com.gamepkg.effects-core-'+('on' if enabled else 'off'), '--target', 'native')
    # Each backend phase must start with unknown achievements so the history
    # failure case forces persistence instead of hitting an already known ID.
    shutil.rmtree(root/'library/progress')
    subprocess.run([executables[0], '--ignored', '--nocapture'], cwd=project, env=dict(environment, GAMEPKG_EFFECT_SDK_NATIVE='1'), check=True, timeout=120)
print('Signed source SDK: core/component/native effect ordering, cache reuse, complete PCM, opt-in denial, premixed-mode denial, failed-tick rollback and explicit browser-export rejection passed.')
