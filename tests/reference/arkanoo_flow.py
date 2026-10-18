"""Compile pinned original constructors/reset/pause and main-loop splash code.

Only SDL window calls and entity allocation are stubbed; state statements and
the original timer condition run unchanged. Maintainer-only fixture generator.
"""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
game = (root / 'src/game.rs').read_text()
main = (root / 'src/main.rs').read_text()
assert hashlib.sha256(game.encode()).hexdigest() == 'd18322f0eac76abc14dfe12a542f0dcf6b154282e3fd8cf5c8c5773d1dc278f1'
assert hashlib.sha256(main.encode()).hexdigest() == '7522df06e4d04bfcc00a2261022e21b18300acc339e394bc22ff83bb95824c73'


def block(source, marker):
    begin = source.index(marker)
    opening = source.index('{', begin)
    depth, end = 1, opening + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[begin:end]


declarations = game[game.index('#[derive(PartialEq'):game.index('#[derive(Clone, Copy)]\npub enum SoundEffect')]
methods = '\n'.join(block(game, '    pub fn ' + name + '(') for name in ['new', 'new_level', 'reset', 'toggle_pause'])
timer = block(main[main.index('// Update splash screen timer'):], 'if game.state == GameState::SplashScreen')
skip = block(main[main.index('Event::KeyDown { keycode: Some(Keycode::Return)'):], 'if game.state == GameState::SplashScreen')
escape = block(main[main.index('Event::KeyDown { keycode: Some(Keycode::Escape)'):], 'if game.state == GameState::LevelEditor')
body = '''#![allow(dead_code,unused_mut,unused_variables)]
const BALL_SIZE:i32=12;
struct Paddle{x:i32,y:i32,width:i32} impl Paddle{fn new()->Self{Self{x:570,y:670,width:140}}}
struct Ball; impl Ball{fn new(_:f32,_:f32)->Self{Self}}
struct Block;struct Bonus;struct Rocket;struct Particle;struct Penguin;
fn create_blocks(_:usize)->Vec<Block>{vec![Block]}
struct Editor{pattern_name_editing:bool}
enum MenuState{Main,Settings}struct Menu{state:MenuState}
struct Sdl;impl Sdl{fn mouse(&self)->Self{Self}fn show_cursor(&self,_:bool){}}
struct Canvas;impl Canvas{fn window_mut(&mut self)->&mut Self{self}fn set_grab(&mut self,_:bool){}}
''' + declarations + '\nimpl Game {\n' + methods + '\n}\n'
body += 'fn timer(game:&mut Game,menu:&mut Menu,mut splash_timer:u64)->u64{let sdl_context=Sdl;let mut canvas=Canvas;' + timer + 'splash_timer}\n'
body += 'fn skip(game:&mut Game,menu:&mut Menu){let sdl_context=Sdl;let mut canvas=Canvas;' + skip + '}\n'
body += 'fn escape(game:&mut Game,menu:&mut Menu,editor:&mut Editor){let sdl_context=Sdl;let mut canvas=Canvas;' + escape + '}\n'
body += '''fn mode(g:&Game,started:bool)->u32{match g.state{GameState::SplashScreen=>7,GameState::Paused=>if started{2}else{0},GameState::Playing=>1,_=>6}}
fn main(){
for frames in [0,1,269,270,271]{let mut g=Game::new();let mut m=Menu{state:MenuState::Main};let mut t=0;for _ in 0..frames{t=timer(&mut g,&mut m,t);}println!("{:?}",[frames,mode(&g,false),t as u32]);}
for before in [0,17,269,270]{let mut g=Game::new();let mut m=Menu{state:MenuState::Main};let mut t=0;for _ in 0..before{t=timer(&mut g,&mut m,t);}skip(&mut g,&mut m);println!("{:?}",[before,mode(&g,false),t as u32]);g.reset();t=timer(&mut g,&mut m,t);println!("{:?}",[before,mode(&g,true),t as u32]);}
for started in [false,true]{let mut g=Game::new();g.state=GameState::LevelEditor;g.score=123;g.lives=2;let mut m=Menu{state:MenuState::Settings};let mut e=Editor{pattern_name_editing:false};escape(&mut g,&mut m,&mut e);println!("{:?}",[mode(&g,started),g.score,g.lives]);g.toggle_pause();println!("{:?}",[mode(&g,started),g.score,g.lives]);}
}'''
with tempfile.TemporaryDirectory(prefix='arkanoo-flow-oracle-') as temp:
    source = Path(temp) / 'main.rs'
    binary = Path(temp) / 'oracle'
    source.write_text(body)
    subprocess.run(['rustc', '--edition=2021', '-Awarnings', str(source), '-o', str(binary)], check=True)
    rows = [json.loads(line) for line in subprocess.check_output([str(binary)], text=True).splitlines()]
print(json.dumps({'revision': '9d613e78c9ed5657ae7fda9206cd4a82a893b7c8', 'startup': rows[:5], 'skip_restart': rows[5:13], 'editor_return': rows[13:]}, indent=2))
