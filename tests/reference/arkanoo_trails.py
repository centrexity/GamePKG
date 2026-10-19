"""Pinned original Ball update and shiny-ball effect points; SDL calls recorded."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
entities = (root / 'src/entities.rs').read_text()
rendering = (root / 'src/rendering.rs').read_text()
assert hashlib.sha256(entities.encode()).hexdigest() == 'ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545'
assert hashlib.sha256(rendering.encode()).hexdigest() == '51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
begin = entities.index('    pub fn update(&mut self, gravity_mode: bool)', entities.index('pub struct Ball'))
update = entities[begin:entities.index('    pub fn rect(&self)', begin)]
begin = rendering.index('fn draw_shiny_ball(')
draw = rendering[rendering.index('{', begin) + 1:rendering.index('    // Draw ball from cache', begin)]
stages = [[13.,0.,0.,3,False], [800/60,0.,0.,12,False], [1400/60,0.,0.,25,False], [800/60,0.,0.,3,False], [799/60,0.,0.,1,False], [12.,4.,20.,3,False], [24.,0.,0.,2,True]]
cases = []
for velocity in [0.,799.5/60,800/60,1399.5/60,1400/60]:
    for frame in [0,17]:
        cases.append({'ball':[500.25,300.75,velocity,0.], 'frame':frame, 'trail':[] if velocity==0 else [[492.25,300.75],[496.25,300.75]]})
cases += [
    {'ball':[-.75,-.75,14.,-6.], 'frame':17, 'trail':[[-4.75,-2.75],[-.75,-.75]]},
    {'ball':[1279.25,719.75,-18.,-6.], 'frame':1000000, 'trail':[[1270.25,717.75]]},
    {'ball':[640.,360.,24.,0.], 'frame':4294967295, 'trail':[[620.,360.],[630.,360.]]},
    {'ball':[640.,360.,24.,0.], 'frame':4294967296, 'trail':[[620.,360.],[630.,360.]]},
    {'ball':[640.,360.,24.,0.], 'frame':18446744073709551615, 'trail':[[620.,360.],[630.,360.]]},
    {'ball':[640.,360.,24.,0.], 'frame':17, 'trail':[[0.,0.],[1270.,710.]]},
]
body = '''#![allow(dead_code,non_snake_case)]
use std::collections::VecDeque;
const WINDOW_WIDTH:u32=1280;const WINDOW_HEIGHT:u32=720;const BALL_SIZE:i32=12;
struct Ball{x:f32,y:f32,vel_x:f32,vel_y:f32,spin:f32,active:bool,attached_to_paddle:bool,launch_timer:u32,trail_positions:VecDeque<(f32,f32)>}
impl Ball {fn launch(&mut self){panic!("not part of trail fixtures");}
''' + update + '''}
fn ball(x:f32,y:f32,vx:f32,vy:f32)->Ball{Ball{x,y,vel_x:vx,vel_y:vy,spin:0.,active:true,attached_to_paddle:false,launch_timer:100,trail_positions:VecDeque::new()}}
mod sdl2{pub mod render{pub enum BlendMode{Blend,None}}}
struct Window;struct Point(i32,i32);impl Point{fn new(x:i32,y:i32)->Self{Self(x,y)}}
struct SdlColor;impl SdlColor{fn RGBA(r:u8,g:u8,b:u8,a:u8)->u32{(r as u32)<<24|(g as u32)<<16|(b as u32)<<8|a as u32}}
struct Canvas<T>{points:Vec<[i64;3]>,color:u32,marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{fn set_blend_mode(&mut self,_:sdl2::render::BlendMode){}fn set_draw_color(&mut self,c:u32){self.color=c;}fn draw_point(&mut self,p:Point)->Result<(),()>{self.points.push([p.0 as i64,p.1 as i64,self.color as i64]);Ok(())}}
fn effects(canvas:&mut Canvas<Window>,ball:&Ball,frame_count:u64){
''' + draw + '}\nfn main(){let mut b=ball(100.,200.,0.,0.);\n'
for vx,vy,spin,frames,attached in stages:
    body += f'b.vel_x={vx}f32;b.vel_y={vy}f32;b.spin={spin}f32;b.attached_to_paddle={str(attached).lower()};'
    body += f'for _ in 0..{frames}{{b.update(false);println!("{{:?}}",b.trail_positions.iter().map(|(x,y)|[*x,*y]).collect::<Vec<_>>());}}\n'
for case in cases:
    x,y,vx,vy = case['ball']
    body += f'let mut b=ball({x}f32,{y}f32,{vx}f32,{vy}f32);'
    for x,y in case['trail']:
        body += f'b.trail_positions.push_back(({x}f32,{y}f32));'
    body += f'let mut c:Canvas<Window>=Canvas{{points:Vec::new(),color:0,marker:std::marker::PhantomData}};effects(&mut c,&b,{case["frame"]});println!("{{:?}}",c.points);\n'
body += '}'
with tempfile.TemporaryDirectory(prefix='arkanoo-trail-oracle-') as temp:
    source,binary=Path(temp)/'main.rs',Path(temp)/'oracle'
    source.write_text(body)
    subprocess.run(['rustc','--edition=2021','-Awarnings',str(source),'-o',str(binary)],check=True)
    rows=iter(json.loads(line) for line in subprocess.check_output([str(binary)],text=True).splitlines())
    trace=[{'action':action,'history':[next(rows) for _ in range(action[3])]} for action in stages]
    for case in cases:case['points']=next(rows)
print(json.dumps({'revision':'9d613e78c9ed5657ae7fda9206cd4a82a893b7c8','trace':trace,'cases':cases},separators=(',',':')))
