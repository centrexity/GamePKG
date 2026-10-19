"""Maintainer oracle for unchanged original launch/life/level transition code."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
entities = (root / 'src/entities.rs').read_text()
game = (root / 'src/game.rs').read_text()
ball = entities[entities.index('pub struct Ball {'):entities.index('#[derive(Clone)]', entities.index('pub struct Ball {'))]
def method(source, name):
    begin = source.index('    pub fn ' + name + '(')
    opening = source.index('{', begin)
    depth = 1
    end = opening + 1
    while depth:
        depth += (source[end] == '{') - (source[end] == '}')
        end += 1
    return source[begin:end]
next_level = method(game, 'next_level')
start_next_level = method(game, 'start_next_level')
paddle = entities[entities.index('pub struct Paddle {'):entities.index('pub struct Ball {')]
resets = [1,9,999]
begin = game.index('// Check if all balls are gone (only if portal is not active)')
end = game.index('// Check if all destroyable blocks', begin)
life = game[begin:end]
launches = [(direction, manual) for direction in range(3) for manual in [False, True]]
finishes = [(1,3,False),(1,2,True),(1,1,True),(1,0,True),(1,2,False),(9,2,True),(10,2,True),(999,2,True)]
losses = [(3,100,False),(2,10,False),(1,0,False),(3,100,True)]
header = '''const WINDOW_WIDTH:u32=1280;const WINDOW_HEIGHT:u32=720;const BALL_SIZE:i32=12;const PADDLE_WIDTH:i32=140;const PADDLE_HEIGHT:i32=22;const PADDLE_SPEED:i32=18;
pub struct Rect;
impl Rect {fn new(_:i32,_:i32,_:u32,_:u32)->Self{Self}}
mod rand {
 pub static DIRECTION:std::sync::atomic::AtomicU32=std::sync::atomic::AtomicU32::new(0);
 pub struct Stub;pub trait Rng {fn gen_range(&mut self,range:std::ops::Range<u32>)->u32;}
 impl Rng for Stub {fn gen_range(&mut self,_:std::ops::Range<u32>)->u32 {DIRECTION.load(std::sync::atomic::Ordering::Relaxed)}}
 pub fn thread_rng()->Stub{Stub}
}
'''
body = header + ball + paddle + '''
#[derive(PartialEq)] enum GameState {Playing,GameOver,Victory,LevelTransition}
struct Penguin;impl Penguin {fn new(_:f32,_:f32)->Self{Self}}
enum SoundEffect {Oh}fn play_sound(_:SoundEffect){}
struct Game {state:GameState,paddle:Paddle,balls:Vec<Ball>,lives:u32,score:u32,current_level:usize,lost_life_this_level:bool,portal_active:bool,penguin:Option<Penguin>,stolen_heart_position:Option<(f32,f32)>,blocks:Vec<u32>,bonuses:Vec<u32>,particles:Vec<u32>,rockets:Vec<u32>,portal_completion_timer:u64,max_speed:f32}
impl Game {
''' + next_level + start_next_level + '\nfn lose(&mut self){' + life + '''}
fn new()->Self {let mut paddle=Paddle::new();paddle.width=180;Self{state:GameState::Playing,paddle,balls:Vec::new(),lives:3,score:0,current_level:1,lost_life_this_level:false,portal_active:false,penguin:None,stolen_heart_position:None,blocks:vec![1],bonuses:vec![1],particles:vec![1],rockets:vec![1],portal_completion_timer:17,max_speed:4000.0}}
}
fn create_blocks(level:usize)->Vec<u32>{vec![level as u32]}
fn main(){
'''
for direction, manual in launches:
    body += f'rand::DIRECTION.store({direction},std::sync::atomic::Ordering::Relaxed);let mut b=Ball::new(634.0,658.0);'
    if manual:
        body += 'b.launch();'
    body += 'for _ in 0..33 {b.update(false);println!("{:?}",[b.x,b.y,b.vel_x,b.vel_y,b.launch_timer as f32,if b.attached_to_paddle{1.0}else{0.0}]);}\n'
for level, lives, lost in finishes:
    body += f'let mut g=Game::new();g.current_level={level};g.lives={lives};g.lost_life_this_level={str(lost).lower()};g.next_level();println!("{{:?}}",[g.lives,if g.state==GameState::Victory{{5}}else{{3}}]);\n'
for lives, score, portal in losses:
    body += f'let mut g=Game::new();g.lives={lives};g.score={score};g.portal_active={str(portal).lower()};g.lose();println!("{{:?}}",[g.lives,g.score,g.balls.len() as u32,if g.lost_life_this_level{{1}}else{{0}},if g.state==GameState::GameOver{{4}}else{{1}}]);'
    body += 'if let Some(b)=g.balls.first(){println!("{:?}",[b.x,b.y,b.vel_x,b.vel_y,b.launch_timer as f32]);}\n'
for level in resets:
    body+=f'let mut g=Game::new();g.current_level={level};g.score=100;g.lives=2;g.paddle.x=100;g.paddle.rocket_ammo=3;g.paddle.bonus_timer=200;g.paddle.ghost_timer=300;g.lost_life_this_level=true;g.portal_active=true;g.start_next_level();let b=&g.balls[0];println!("{{:?}}",[g.current_level as f32,g.score as f32,g.lives as f32,g.paddle.x as f32,g.paddle.width as f32,g.paddle.rocket_ammo as f32,g.paddle.bonus_timer as f32,g.paddle.ghost_timer as f32,g.balls.len() as f32,b.x,b.y,b.launch_timer as f32,g.bonuses.len() as f32,g.rockets.len() as f32,g.particles.len() as f32,if g.lost_life_this_level{{1.0}}else{{0.0}},if g.portal_active{{1.0}}else{{0.0}},g.portal_completion_timer as f32,g.max_speed]);\n'
body += '}\n'
with tempfile.TemporaryDirectory(prefix='arkanoo-transition-reference-') as temporary:
    directory=Path(temporary);rust=directory/'reference.rs';rust.write_text(body);binary=directory/'reference'
    subprocess.run(['rustc','--edition=2021','-O',str(rust),'-o',str(binary)],check=True)
    lines=iter(subprocess.check_output([str(binary)],text=True).splitlines())
    launch_fixtures=[dict(direction=direction,manual=manual,expected=[json.loads(next(lines)) for _ in range(33)]) for direction,manual in launches]
    finish_fixtures=[dict(level=level,lives=lives,lost=lost,expected=json.loads(next(lines))) for level,lives,lost in finishes]
    loss_fixtures=[]
    for lives,score,portal in losses:
        expected=json.loads(next(lines));fixture=dict(lives=lives,score=score,portal=portal,expected=expected)
        if expected[2]:fixture['ball']=json.loads(next(lines))
        loss_fixtures.append(fixture)
    reset_fixtures=[dict(level=level,expected=json.loads(next(lines))) for level in resets]
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',ball_sha256=hashlib.sha256(ball.encode()).hexdigest(),next_level_sha256=hashlib.sha256(next_level.encode()).hexdigest(),life_sha256=hashlib.sha256(life.encode()).hexdigest(),launches=launch_fixtures,finishes=finish_fixtures,losses=loss_fixtures,resets=reset_fixtures,start_next_level_sha256=hashlib.sha256(start_next_level.encode()).hexdigest()),indent=2))
