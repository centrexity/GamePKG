"""Prepare a PRIVATE full-port recorded-audio candidate; never modifies published assets.
Maintainer Python only. All music/effects remain package-owned data.
"""
from pathlib import Path
import argparse, hashlib, json, shutil
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--private-output',type=Path,required=True)
parser.add_argument('--migration-inputs',type=Path,required=True)
args=parser.parse_args();project=Path(__file__).resolve().parents[1];out=args.private_output.resolve()
assert project not in out.parents and out!=project and not out.exists(),'Require new private output outside workspace'
inputs=args.migration_inputs;metadata=json.loads((inputs/'migration-inputs.json').read_text())
assert len(metadata['effects'])==5 and len(metadata['music'])==9,'Require complete original dataset'
paths=[item['asset'] for item in metadata['effects']+metadata['music']]
assert len(set(paths))==14,'Duplicate recording path'
for name in paths:
 assert isinstance(name,str) and not Path(name).is_absolute() and '..' not in Path(name).parts and '\\' not in name,'Invalid recording path'
 assert name.startswith(('audio/','music/')) and len(Path(name).parts)==2,'Invalid recording directory'
assert (inputs/'audio/music-list.txt').read_text()==''.join(item['asset']+'\n' for item in metadata['music']),'Playlist must retain every track in order'
for item in metadata['effects']+metadata['music']:
 path=inputs/item['asset'];assert not path.is_symlink() and path.is_file()
 expected=item.get('wav_sha256',item.get('source_sha256'))
 assert hashlib.sha256(path.read_bytes()).hexdigest()==expected,path.name
shutil.copytree(project/'ports/arkanoo',out)
for item in metadata['effects']+metadata['music']:
 target=out/'assets'/item['asset'];target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(inputs/item['asset'],target)
shutil.copy2(inputs/'audio/music-list.txt',out/'assets/audio/music-list.txt')
def patch(file,old,new):
 path=out/'source/src'/file;s=path.read_text();assert s.count(old)==1,(file,old);path.write_text(s.replace(old,new))
manifest=out/'manifest.toml';manifest.write_text(manifest.read_text().replace('[limits]','[limits]\neffects_memory_bytes = 3145728\nmusic_memory_bytes = 7340032'))
(out/'source/src/audio.rs').write_text('''// Host-owned original recordings; no guest decoder or premixed PCM.
pub struct Mixer {}
impl Mixer {
    pub fn new() -> Self { Self {} }
    pub fn effect_request(&mut self, kind:u32, _frame:u64, muted:bool)->bool {
        if muted || kind<1 || kind>5 { return false; }
        let path=match kind {1=>"audio/bounce.wav",2=>"audio/glass.wav",3=>"audio/launch.wav",4=>"audio/explosion.wav",_=>"audio/life-loss.wav"};
        crate::gamepkg::effect_play(path)
    }
}
''')
patch('menu.rs','    pub sfx_muted: bool,','    pub sfx_muted: bool,\n    pub music_volume:u32,\n    pub music_muted:bool,\n    pub dragging_music:bool,')
patch('menu.rs','            sfx_muted: false,','            sfx_muted: false,\n            music_volume:64,\n            music_muted:false,\n            dragging_music:false,')
patch('menu.rs','        } else if action == 10 {','        } else if action == 12 {\n            [270.0, 170.0, 100.0, 20.0]\n        } else if action == 10 {')
patch('menu.rs','        if self.settings && (270.0..370.0).contains(&x) && (215.0..235.0).contains(&y) {','        if self.settings && (270.0..370.0).contains(&x) && (170.0..190.0).contains(&y) { return 12; }\n        if self.settings && (270.0..370.0).contains(&x) && (215.0..235.0).contains(&y) {')
# Both original sliders acquire on held motion and retain drag until release.
p=out/'source/src/menu.rs';s=p.read_text().replace('self.dragging_volume = false;','self.dragging_volume = false;\n            self.dragging_music = false;');p.write_text(s)
patch('menu.rs','                if self.dragging_volume {','                let mut changed=false;\n                if (540..740).contains(&original_x) && (270..290).contains(&original_y) { self.dragging_music=true; }\n                if self.dragging_music {\n                    let next=((original_x-540).clamp(0,200)*128/200) as u32;\n                    changed=next!=self.music_volume;self.music_volume=next;\n                }\n                if self.dragging_volume {')
patch('menu.rs','                        return 11;','                        changed=true;')
patch('menu.rs','            }\n        }\n        if kind == 2 && code == 1 {\n            return self.hover;','                if changed { return 11; }\n            }\n        }\n        if kind == 2 && code == 1 {\n            return self.hover;')
patch('physics.rs','            if action == 10 {','            if action == 12 { self.menu.music_muted=!self.menu.music_muted; self.dirty=true; }\n            if action == 10 {')
patch('physics.rs','        let sfx_volume = self.menu.sfx_volume;','        let music_volume=self.menu.music_volume;\n        let music_muted=self.menu.music_muted;\n        let sfx_volume = self.menu.sfx_volume;')
patch('physics.rs','        self.menu.sfx_volume = sfx_volume;','        self.menu.music_volume=music_volume;\n        self.menu.music_muted=music_muted;\n        self.menu.sfx_volume = sfx_volume;')
patch('physics.rs','        if self.sound != 0 {','        if self.sound != 0 {\n            crate::gamepkg::effect_volume(if self.menu.sfx_muted {0}else{self.menu.sfx_volume});')
patch('render.rs','for action in [10, 7, 8]','for action in [12, 10, 7, 8]')
patch('render.rs','        draw_sfx_slider(game.menu.sfx_volume);','        draw_sfx_slider(game.menu.sfx_volume);\n        draw_volume_slider(game.menu.music_volume,195.0);')
patch('render.rs','pub fn draw_sfx_slider(volume: u32) {','pub fn draw_sfx_slider(volume:u32) {draw_volume_slider(volume,240.0);}\npub fn draw_volume_slider(volume: u32, top:f32) {')
p=out/'source/src/render.rs';s=p.read_text();start=s.index('pub fn draw_volume_slider');end=s.index('pub fn draw_menu_button',start);part=s[start:end].replace('240.0','top').replace('249.5','top+9.5').replace('            360,','            ((top-60.0)*2.0) as i32,');p.write_text(s[:start]+part+s[end:])
patch('render.rs','        10 => {','        12 => {if game.menu.music_muted {"Music: OFF"}else{"Music: ON"}}\n        10 => {')
patch('lib.rs','mod audio;','mod audio;\nmod music;')
patch('lib.rs','static GAME: Mutex<Option<physics::Game>> = Mutex::new(None);','static GAME: Mutex<Option<physics::Game>> = Mutex::new(None);\nstatic MUSIC: Mutex<Option<music::Music>> = Mutex::new(None);')
patch('lib.rs','    if let Ok(mut slot) = GAME.lock() {','    if let Ok(mut slot) = GAME.lock() {\n        let Ok(mut music_slot)=MUSIC.lock() else {return state;};\n        if let Some(manager)=music_slot.as_mut() {manager.begin_tick();}')
patch('lib.rs','            let mut game = physics::Game::new();','            let mut manager=music::Music::from_package();\n            manager.begin_tick();manager.play_music();\n            let mut game = physics::Game::new();')
patch('lib.rs','                            game.menu.sfx_muted = game.editor.sfx_muted;','                            game.menu.sfx_muted = game.editor.sfx_muted;\n                            game.menu.music_volume=game.editor.music_volume;\n                            game.menu.music_muted=game.editor.music_muted;')
patch('lib.rs','            *slot = Some(game);','            manager.set_music_volume(game.menu.music_volume as i32);\n            manager.set_music_muted(game.menu.music_muted);manager.play_music();\n            *music_slot=Some(manager);\n            *slot = Some(game);')
patch('lib.rs','            if let Some(samples) = game.effects.effect_render(game.menu.sfx_volume, game.menu.sfx_muted) {\n                gamepkg::audio_submit_stereo(&samples);\n            }','            gamepkg::effect_volume(if game.menu.sfx_muted {0}else{game.menu.sfx_volume});\n            if let Some(manager)=music_slot.as_mut() {\n                if manager.get_music_volume()!=game.menu.music_volume as i32 {manager.set_music_volume(game.menu.music_volume as i32);}\n                if manager.is_music_muted()!=game.menu.music_muted {manager.set_music_muted(game.menu.music_muted);}\n                manager.update();\n                if game.quit_requested {manager.stop_music();}\n            }')
patch('lib.rs','game.editor.save_encode_settings(','game.editor.save_encode_audio_settings(')
patch('lib.rs','                    game.menu.sfx_volume,\n                    game.menu.sfx_muted,','                    [game.menu.sfx_volume,game.menu.music_volume],\n                    [game.menu.sfx_muted,game.menu.music_muted],')
print('Private complete recorded-audio port candidate:',out)
print('All five original effects and nine music tracks retained; existing hard memory limit unchanged.')
