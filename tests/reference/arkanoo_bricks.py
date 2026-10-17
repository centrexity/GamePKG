"""Maintainer oracle: unchanged original brick/rocket phases with actual SDL."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile

root=Path(sys.argv[1]);entities=(root/'src/entities.rs').read_text();game=(root/'src/game.rs').read_text()
def method(source,name):
    begin=source.index('pub fn '+name+'(');opening=source.index('{',begin);depth=1;end=opening+1
    while depth:
        depth+=(source[end]=='{')-(source[end]=='}');end+=1
    return source[begin:end]
types=entities[entities.index('#[derive(Clone, Copy, PartialEq)]'):entities.index('pub struct Paddle')]
block=entities[entities.index('#[derive(Clone)]'):entities.index('pub struct Bonus')]
rocket=entities[entities.index('pub struct Rocket'):entities.index('/// Particle for glass')]
ball_rect=method(entities[entities.index('impl Ball {'):],'rect')
check=method(entities,'check_collision')
begin=game.index('// Block collision');end=game.index('if portal_just_activated',begin);collision=game[begin:end]
begin=game.index('// Update Rockets');end=game.index('// Create all queued particles',begin);rockets=game[begin:end]
layout=[[40,80,0],[100,80,3],[101,80,1],[40,140,1],[40,141,0],[76,128,0],[77,128,0],[160,80,2],[161,80,0],[40,200,3],[40,201,0],[112,176,1],[113,176,0],[220,80,0]]
explosive=[item.copy() for item in layout];explosive[0][2]=2
cases=[
 dict(name='normal',blocks=[[40,80,0]],ball=[50.,76.,0.,4.],ghost=False,repeats=1),
 dict(name='ice-two-hits',blocks=[[40,80,1]],ball=[50.,76.,0.,4.],ghost=False,repeats=2),
 dict(name='metal',blocks=[[40,80,3]],ball=[50.,76.,0.,4.],ghost=False,repeats=2),
 dict(name='ghost-mixed',blocks=[[40,80,0],[40,80,3],[40,80,1]],ball=[50.,76.,0.,4.],ghost=True,repeats=2),
 dict(name='metal-stops-first',blocks=[[40,80,3],[40,80,0]],ball=[50.,76.,0.,4.],ghost=False,repeats=1),
 dict(name='explosive-radius-no-chain',blocks=explosive,ball=[50.,76.,0.,4.],ghost=False,repeats=1),
 dict(name='rocket-radius-no-chain',blocks=layout,rocket=[[50.,104.]],repeats=1),
 dict(name='rocket-metal-direct',blocks=[[40,80,3],[100,80,1]],rocket=[[50.,104.]],repeats=1),
 dict(name='rocket-two-same-target',blocks=layout,rocket=[[50.,104.],[50.,104.]],repeats=1),
 dict(name='rocket-above-top',blocks=layout,rocket=[[0.,5.]],repeats=1),
]
header='''const BALL_SIZE:i32=12;const BLOCK_WIDTH:i32=60;const BLOCK_HEIGHT:i32=20;
#[repr(C)] pub struct Rect{x:i32,y:i32,w:i32,h:i32}
#[link(name="SDL2")] extern "C" {fn SDL_IntersectRect(a:*const Rect,b:*const Rect,r:*mut Rect)->i32;fn SDL_HasIntersection(a:*const Rect,b:*const Rect)->i32;}
impl Rect {
 fn new(x:i32,y:i32,w:u32,h:u32)->Self {Self{x,y,w:w as i32,h:h as i32}}
 fn width(&self)->u32{self.w as u32}fn height(&self)->u32{self.h as u32}
 fn intersection(&self,other:Self)->Option<Self>{let mut r=Self::new(0,0,0,0);if unsafe{SDL_IntersectRect(self,&other,&mut r)}!=0{Some(r)}else{None}}
 fn has_intersection(&self,other:Self)->bool{(unsafe{SDL_HasIntersection(self,&other)})!=0}
}
mod rand {pub struct Stub;pub trait Sample{fn sample()->Self;}impl Sample for f32{fn sample()->Self{0.99}}
 pub trait Rng{fn gen<T:Sample>(&mut self)->T;fn gen_range(&mut self,range:std::ops::Range<i32>)->i32;}
 impl Rng for Stub{fn gen<T:Sample>(&mut self)->T{T::sample()}fn gen_range(&mut self,_:std::ops::Range<i32>)->i32{0}}
 pub fn thread_rng()->Stub{Stub}
}
use rand::Rng;
struct Ball{x:f32,y:f32,vel_x:f32,vel_y:f32,active:bool}
'''
body=header+'impl Ball{'+ball_rect+'}\n'+types+block+rocket+check+'''
enum SoundEffect{Bounce,BreakingGlass,Explosion}fn play_sound(_:SoundEffect){}
struct Paddle{ghost_timer:u32}struct Bonus;impl Bonus{fn new(_:f32,_:f32,_:BonusType)->Self{Self}}
struct Game{blocks:Vec<Block>,balls:Vec<Ball>,rockets:Vec<Rocket>,paddle:Paddle,bonuses:Vec<Bonus>,bonus_cooldown:u64,score:u32}
impl Game{fn collisions(&mut self){let mut particles_to_spawn=Vec::new();for ball in &mut self.balls {
'''+collision+'}\nfn rockets(&mut self){let mut particles_to_spawn=Vec::new();\n'+rockets+'}\n}\nfn main(){\n'
kind=['Normal','Ice','Explosive','Undestroyable']
for case in cases:
    blocks=','.join(f'Block::new({x},{y},Color::new(1,2,3),BlockType::{kind[k]})' for x,y,k in case['blocks'])
    rocket_values=','.join(f'Rocket::new({x}f32,{y}f32)' for x,y in case.get('rocket',[]))
    body+='let mut g=Game{blocks:vec!['+blocks+'],balls:Vec::new(),rockets:vec!['+rocket_values+'],paddle:Paddle{ghost_timer:'+str(1 if case.get('ghost') else 0)+'},bonuses:Vec::new(),bonus_cooldown:60,score:0};\n'
    for _ in range(case['repeats']):
        if 'ball' in case:
            x,y,vx,vy=case['ball'];body+=f'g.balls=vec![Ball{{x:{x}f32,y:{y}f32,vel_x:{vx}f32,vel_y:{vy}f32,active:true}}];g.collisions();\n'
        else:body+='g.rockets();\n'
        body+='println!("{:?}",[g.score,g.rockets.iter().filter(|r|r.active).count() as u32,g.bonuses.len() as u32]);for b in &g.blocks{println!("{:?}",[if b.active{1}else{0},b.health as u32]);}for b in &g.balls{println!("{:?}",[b.x,b.y,b.vel_x,b.vel_y]);}for r in &g.rockets{if r.active{println!("{:?}",[r.x,r.y]);}}\n'
body+='}\n'
with tempfile.TemporaryDirectory(prefix='arkanoo-brick-reference-') as temporary:
    directory=Path(temporary);rust=directory/'reference.rs';rust.write_text(body);binary=directory/'reference'
    subprocess.run(['rustc','--edition=2021','-O',str(rust),'-o',str(binary)],check=True)
    lines=iter(subprocess.check_output([str(binary)],text=True).splitlines())
    for case in cases:
        case['expected']=[]
        for _ in range(case['repeats']):
            state=json.loads(next(lines));blocks=[json.loads(next(lines)) for _ in case['blocks']]
            expected=dict(state=state,blocks=blocks)
            if 'ball' in case:expected['ball']=json.loads(next(lines))
            expected['rockets']=[json.loads(next(lines)) for _ in range(state[1])]
            case['expected'].append(expected)
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',entities_sha256=hashlib.sha256(entities.encode()).hexdigest(),game_sha256=hashlib.sha256(game.encode()).hexdigest(),collision_sha256=hashlib.sha256(collision.encode()).hexdigest(),rockets_sha256=hashlib.sha256(rockets.encode()).hexdigest(),fixtures=cases),indent=2))
