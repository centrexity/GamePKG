"""Maintainer oracle: original ball update plus actual SDL rectangle intersections."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile

source = Path(sys.argv[1]) / 'src/entities.rs'
text = source.read_text()
begin = text.index('    pub fn update(&mut self, gravity_mode: bool)')
end = text.index('    pub fn rect(&self)', begin)
fragment = text[begin:end]
paddle_fragment = text[text.index('pub struct Paddle {'):text.index('pub struct Ball {')]
game_source = (Path(sys.argv[1]) / 'src/game.rs').read_text()
resolve_begin = game_source.index('// Collision Resolution')
resolve_end = game_source.index('if destroyed {', resolve_begin)
resolve_fragment = game_source[resolve_begin:resolve_end]
block_cases = [(False,[30.,84.,4.,2.]),(False,[99.,84.,-4.,2.]),(False,[50.,70.,4.,2.]),(False,[50.,99.,4.,-2.]),(False,[30.,70.,4.,2.]),(False,[30.9,70.9,4.,2.]),(True,[30.9,70.9,4.,2.]),(False,[100.,84.,-4.,2.])]
paddle_cases = [[['left',1],['both',1],['long',1],['step',299],['step',1],['step',1]], [['left',100],['both',1],['long',1],['step',300],['both',1]], [['right',100],['long',1],['step',1],['step',299],['step',1]], [['long',1],['step',50],['long',1],['step',299],['step',1],['ghost',1],['step',600]]]
cases = [
    (False,[1.,300.,-4.,-2.,0.],4),
    (False,[1267.,300.,4.,-2.,0.],4),
    (False,[300.,1.,4.,-4.,0.],4),
    (False,[300.,718.,4.,4.,0.],4),
    (False,[300.,400.,0.,-4.,0.],10),
    (False,[300.,400.,-0.5,-4.,0.],10),
    (False,[300.,400.,4.,-2.,6.],60),
    (True,[300.,100.,4.,2.,6.],60),
    (True,[300.,100.,4.,12.,6.],60),
    (True,[300.,400.,4.,-4.,6.],60),
]
rectangles = [
    ([40.49,80.9,12,12],[40,80,60,20]),
    ([99.9,85.2,12,12],[40,80,60,20]),
    ([100.,85.2,12,12],[40,80,60,20]),
    ([50.25,99.9,12,12],[40,80,60,20]),
    ([50.25,100.,12,12],[40,80,60,20]),
    ([-0.9,-0.9,12,12],[0,0,60,20]),
    ([630.9,658.9,12,12],[570,670,140,22]),
    ([630.9,659.,12,12],[570,670,140,22]),
    ([95.5,78.5,10,20],[40,80,60,20]),
    ([699.9,669.9,40,40],[570,670,140,22]),
]
header = '''use std::collections::VecDeque;
const WINDOW_WIDTH:u32=1280;const WINDOW_HEIGHT:u32=720;const BALL_SIZE:i32=12;
struct Ball {x:f32,y:f32,vel_x:f32,vel_y:f32,spin:f32,active:bool,attached_to_paddle:bool,launch_timer:u32,trail_positions:VecDeque<(f32,f32)>}
impl Ball {fn launch(&mut self){panic!("unattached fixtures only");}
'''
body=header+fragment+'}\n'+paddle_fragment+'\nconst PADDLE_WIDTH:i32=140;const PADDLE_SPEED:i32=18;const PADDLE_HEIGHT:i32=22;\n'
body+='struct Block{x:i32,y:i32} struct Game{paddle:Paddle} impl Game {fn resolve(&self,ball:&mut Ball,block:&Block,overlap:&Rect){'+resolve_fragment+'}}\nconst BLOCK_WIDTH:i32=60;const BLOCK_HEIGHT:i32=20;\n'
body+='''#[repr(C)] pub struct Rect { x:i32,y:i32,w:i32,h:i32 }
#[link(name="SDL2")] extern "C" {fn SDL_IntersectRect(a:*const Rect,b:*const Rect,result:*mut Rect)->i32;}
impl Rect {fn width(&self)->u32 {self.w as u32} fn height(&self)->u32 {self.h as u32} fn new(x:i32,y:i32,w:u32,h:u32)->Self {Self{x,y,w:w as i32,h:h as i32}}}
fn main(){
'''
for gravity, values, steps in cases:
    x,y,vx,vy,spin = values
    body += f'let mut b=Ball{{x:{x}f32,y:{y}f32,vel_x:{vx}f32,vel_y:{vy}f32,spin:{spin}f32,active:true,attached_to_paddle:false,launch_timer:0,trail_positions:VecDeque::new()}};\n'
    body += f'for _ in 0..{steps} {{b.update({str(gravity).lower()});println!("{{:?}}",[b.x,b.y,b.vel_x,b.vel_y,b.spin,if b.active {{1.0}} else {{0.0}}]);}}\n'
for a,b in rectangles:
    def rect(values):
        x,y,w,h=values
        return f'Rect{{x:{x}f32 as i32,y:{y}f32 as i32,w:{w},h:{h}}}'
    body += f'let a={rect(a)};let b={rect(b)};let mut r=Rect{{x:0,y:0,w:0,h:0}};let hit=unsafe{{SDL_IntersectRect(&a,&b,&mut r)}};println!("{{:?}}",if hit!=0 {{[r.w,r.h]}} else {{[0,0]}});\n'
for events in paddle_cases:
    body+='let mut p=Paddle::new();\n'
    for action,frames in events:
        statements={'left':'p.move_left();','right':'p.move_right();','both':'p.move_left();p.move_right();','long':'p.activate_long_bonus();','ghost':'p.activate_ghost_bonus();','step':''}[action]
        body+=f'for _ in 0..{frames}{{{statements}p.update();}}println!("{{:?}}",[p.x,p.width,p.vel_x,p.bonus_timer as i32,p.ghost_timer as i32]);\n'
for ghost,values in block_cases:
    x,y,vx,vy=values
    body+=f'let mut b=Ball{{x:{x}f32,y:{y}f32,vel_x:{vx}f32,vel_y:{vy}f32,spin:0.0,active:true,attached_to_paddle:false,launch_timer:0,trail_positions:VecDeque::new()}};let mut g=Game{{paddle:Paddle::new()}};g.paddle.ghost_timer={1 if ghost else 0};let block=Block{{x:40,y:80}};let a=Rect::new(b.x as i32,b.y as i32,12,12);let r=Rect::new(40,80,60,20);let mut intersection=Rect::new(0,0,0,0);let hit=unsafe{{SDL_IntersectRect(&a,&r,&mut intersection)}};if hit!=0{{g.resolve(&mut b,&block,&intersection);}}println!("{{:?}}",[b.x,b.y,b.vel_x,b.vel_y,if hit!=0{{1.0}}else{{0.0}}]);\n'
body+='}\n'
with tempfile.TemporaryDirectory(prefix='arkanoo-world-reference-') as temporary:
    root=Path(temporary);rust=root/'reference.rs';rust.write_text(body);binary=root/'reference'
    subprocess.run(['rustc','--edition=2021','-O',str(rust),'-o',str(binary)],check=True)
    lines=iter(subprocess.check_output([str(binary)],text=True).splitlines())
    fixtures=[]
    for gravity,values,steps in cases:
        fixtures.append(dict(gravity=gravity,ball=values,expected=[json.loads(next(lines)) for _ in range(steps)]))
    intersections=[dict(first=a,second=b,expected=json.loads(next(lines))) for a,b in rectangles]
    paddles=[dict(events=events,expected=[json.loads(next(lines)) for _ in events]) for events in paddle_cases]
    blocks=[dict(ghost=ghost,ball=values,expected=json.loads(next(lines))) for ghost,values in block_cases]
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),paddle_fragment_sha256=hashlib.sha256(paddle_fragment.encode()).hexdigest(),fixtures=fixtures,intersections=intersections,paddles=paddles,blocks=blocks,resolve_fragment_sha256=hashlib.sha256(resolve_fragment.encode()).hexdigest()),indent=2))
