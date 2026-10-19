"""Maintainer-only generated music SDK fixtures. No player tools or keys required."""
from pathlib import Path
import json, os, shutil, subprocess, tempfile, wave
project = Path(__file__).resolve().parents[1]
cli = project/'target/release/gamepkg'
decoder = project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'
env = dict(os.environ, GAMEPKG_BUNDLED_AUDIO_DECODER=str(decoder))
result = subprocess.run(['cargo','build','--locked','--release','--bin','gamepkg'],cwd=project,env=env,capture_output=True,text=True,timeout=300)
assert result.returncode == 0, result.stderr
result = subprocess.run(['cargo','test','--locked','--release','--features','audio','--test','music_sdk','--no-run','--message-format=json'],cwd=project,env=env,capture_output=True,text=True,timeout=300)
assert result.returncode == 0, result.stderr
artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
executable = next(item['executable'] for item in artifacts if item.get('target',{}).get('name')=='music_sdk' and item.get('executable'))
with tempfile.TemporaryDirectory(prefix='gamepkg-music-sdk-') as temporary:
    root = Path(temporary)
    environment = dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_MUSIC_SDK_LIBRARY=str(root/'library'),GAMEPKG_MUSIC_SDK_REFERENCE=str(root/'reference.s16'))
    (root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    (root/'null-alsa.conf').write_text('pcm.!default { type null }\n')
    environment['ALSA_CONFIG_PATH']=str(root/'null-alsa.conf')
    key=root/'synthetic.key'; key.write_text('69'*32)
    with wave.open(str(root/'effect.wav'),'wb') as output:
        output.setnchannels(2); output.setsampwidth(2); output.setframerate(44100)
        output.writeframes((int(12345).to_bytes(2,'little',signed=True)+int(-23456).to_bytes(2,'little',signed=True))*4096)
    subprocess.run(['ffmpeg','-v','error','-i',str(root/'effect.wav'),'-codec:a','libmp3lame','-write_xing','0',str(root/'music.mp3')],check=True)
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_mixer','sdl2'],text=True).split()
    subprocess.run(['cc',str(project/'tools/audio-decoder/reference.c'),'-o',str(root/'reference'),*flags],check=True)
    with (root/'reference.s16').open('wb') as output:
        subprocess.run([str(root/'reference'),str(root/'music.mp3'),'44100','44100'],stdout=output,check=True)
    def run(*arguments,success=True):
        result=subprocess.run([str(cli),*map(str,arguments)],env=environment,capture_output=True,text=True,timeout=180)
        assert (result.returncode==0)==success,result.stdout+'\n'+result.stderr
        return result
    for profile,suffix in [('core-v1','core'),('component-v1','component')]:
        for label in ['off','on','solo']:
            enabled = label != 'off'
            id=f'com.gamepkg.music-{suffix}-'+label
            recipe=root/id; shutil.copytree(project/'examples/minimal',recipe)
            text=(recipe/'manifest.toml').read_text().replace('com.gamepkg.minimal',id).replace('audio = false','audio = true').replace('save = false','save = true')
            if enabled:text=text.replace('[limits]','[limits]\nmusic_memory_bytes=7340032'+('\neffects_memory_bytes=1048576' if label=='on' else ''))
            text+='\n[[achievements]]\nid="first"\ntitle="First"\ndescription="Fixture"\n[[achievements]]\nid="second"\ntitle="Second"\ndescription="Fixture"\n[[achievements]]\nid="third"\ntitle="Third"\ndescription="Fixture"\n'
            (recipe/'manifest.toml').write_text('runtime_profile="'+profile+'"\n'+text)
            for name in ['effect.wav','music.mp3']:shutil.copy2(root/name,recipe/'assets'/name)
            (recipe/'source/src/lib.rs').write_text('''pub fn game_tick(input:u32,_:u64)->u64 {
                if input==3 {return u64::from(gamepkg::music_finished());}
                if input==4 {return u64::from(gamepkg::music_failed());}
                if input==5 {return u64::from(gamepkg::music_play("missing.mp3"));}
                if input==6 {
                    gamepkg::music_stop();gamepkg::music_pause(true);
                    gamepkg::effect_play("effect.wav");return 0;
                }
                let mut result=0u64;
                if gamepkg::music_play("music.mp3") {result|=1;}
                if gamepkg::music_volume(64) {result|=2;}
                if gamepkg::music_pause(false) {result|=4;}
                if gamepkg::music_stop() {result|=8;}
                if gamepkg::music_play("music.mp3") {result|=16;}
                if !gamepkg::music_volume(129) {result|=32;}
                if !gamepkg::music_play("../music.mp3") {result|=64;}
                if !gamepkg::audio_submit(&[]) {result|=128;}
                if gamepkg::save_write(&[1]) {result|=256;}
                if gamepkg::achievement_unlock(input) {result|=512;}
                if gamepkg::effect_play("effect.wav") {result|=1024;}
                let divisor:u32=if input==1 {0}else{1};result+u64::from(10/divisor)
            }''')
            archive=root/(id+'.gamepkg')
            run('pack',recipe,'-o',archive,'--key',key);run('install',archive,'--trust-creator');run('build',id,'--target','wasm')
            if enabled:assert 'music output is not yet available' in run('web-export',id,'--output',root/(id+'-web'),success=False).stderr
    subprocess.run([executable,'--ignored','--nocapture'],cwd=project,env=environment,check=True,timeout=120)
    for label in ['off','on','solo']:run('build','com.gamepkg.music-core-'+label,'--target','native')
    shutil.rmtree(root/'library/progress')
    subprocess.run([executable,'--ignored','--nocapture'],cwd=project,env=dict(environment,GAMEPKG_MUSIC_SDK_NATIVE='1'),check=True,timeout=120)
print('Signed no-dependency music SDK: native/core/component, opt-in, shared memory, original-rate PCM, completion and trap/save/history/output rollback passed.')
