"""Maintainer-only game source-policy/build check for the port music controller."""
from pathlib import Path
import os,shutil,subprocess,tempfile
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-port-music-source-') as temporary:
 root=Path(temporary);recipe=root/'recipe';shutil.copytree(project/'examples/minimal',recipe)
 (recipe/'source/src/music.rs').write_bytes((project/'ports/arkanoo/source/src/music.rs').read_bytes())
 (recipe/'source/src/lib.rs').write_text('''mod music;
mod physics {pub fn below(bound:u64)->u64 {let threshold=bound.wrapping_neg()%bound;loop{let value=crate::gamepkg::random_u64();if value>=threshold{return value%bound;}}}}
pub fn game_tick(_:u32,state:u64)->u64 {let mut manager=music::Music::from_package();manager.begin_tick();manager.play_music();manager.set_music_volume(64);manager.set_music_muted(true);manager.toggle_music_mute();manager.update();manager.stop_music();gamepkg::draw_clear(if manager.is_music_muted(){0xff0000ff}else{0x00ff00ff});state+manager.get_music_volume() as u64+1}
''')
 (recipe/'assets/audio').mkdir();(recipe/'assets/audio/music-list.txt').write_text('music/fixture.mp3\n');(recipe/'assets/music').mkdir();(recipe/'assets/music/fixture.mp3').write_bytes(b'synthetic source-only fixture')
 p=recipe/'manifest.toml';p.write_text(p.read_text().replace('com.gamepkg.minimal','com.gamepkg.music-controller-source').replace('audio = false','audio = true').replace('[limits]','[limits]\nmusic_memory_bytes=7340032'))
 (root/'key').write_text('52'*32);(root/'settings.toml').write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n');env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'))
 def run(*args):
  result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=180);assert result.returncode==0,result.stdout+'\n'+result.stderr
 run('pack',recipe,'--key',root/'key','-o',root/'fixture.gamepkg');run('install',root/'fixture.gamepkg','--trust-creator')
 for target in ['wasm','native']:run('build','com.gamepkg.music-controller-source','--target',target);print(target,'approved music controller source validation and isolated build passed',flush=True)
