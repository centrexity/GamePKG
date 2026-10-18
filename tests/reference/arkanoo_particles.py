"""Run unchanged pinned particle lifecycle and shard drawing (SDL calls recorded)."""
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
particle = entities[entities.index('pub struct Particle {'):entities.index('/// Penguin animation states')]
begin = rendering.index('fn draw_particle(')
end = rendering.index('/// Draw animated penguin', begin)
draw = rendering[begin:end]
body = '''#![allow(dead_code,non_snake_case)]
struct Color{r:u8,g:u8,b:u8}
mod rand {
 pub struct Stub;pub fn thread_rng()->Stub{Stub}
 pub trait Sample:Sized{fn sample(range:std::ops::Range<Self>)->Self;}
 impl Sample for f32{fn sample(r:std::ops::Range<Self>)->Self{r.start+(r.end-r.start)*0.25}}
 impl Sample for u32{fn sample(r:std::ops::Range<Self>)->Self{r.start}}
 impl Sample for i32{fn sample(r:std::ops::Range<Self>)->Self{r.start}}
 pub trait Rng{fn gen_range<T:Sample>(&mut self,r:std::ops::Range<T>)->T{T::sample(r)}}
 impl Rng for Stub{}
}
struct Window;struct Point(i32,i32);impl Point{fn new(x:i32,y:i32)->Self{Self(x,y)}}
struct SdlColor;impl SdlColor{fn RGBA(r:u8,g:u8,b:u8,a:u8)->[u8;4]{[r,g,b,a]}}
struct Canvas<T>{points:Vec<[i32;2]>,color:[u8;4],marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{fn set_draw_color(&mut self,c:[u8;4]){self.color=c;}fn draw_point(&mut self,p:Point)->Result<(),()>{self.points.push([p.0,p.1]);Ok(())}}
''' + particle + draw + '''
fn main(){
let p=Particle::new(12.5,-3.25,2.0,-4.0,Color{r:25,g:100,b:200});
println!("{:?}",[p.rotation,p.rotation_speed,p.max_lifetime as f32,p.size as f32]);
for (x,y,vx,vy,rotation,spin,life,size) in [
 (100.75,200.25,2.0,-4.0,0.0,7.5,20,3),
 (-0.75,0.75,-2.5,3.0,45.0,-10.0,39,7),
 (1279.5,719.75,0.0,0.0,90.0,0.0,30,6),
 (640.0,360.0,1.5,-2.25,359.0,9.5,21,5)]{
 let mut p=Particle{x,y,vel_x:vx,vel_y:vy,rotation,rotation_speed:spin,lifetime:0,max_lifetime:life,size,color:Color{r:25,g:100,b:200}};
 println!("{:?}",[x,y,vx,vy,rotation,spin,life as f32,size as f32]);
 for _ in 0..=life {
  let mut c:Canvas<Window>=Canvas{points:Vec::new(),color:[0;4],marker:std::marker::PhantomData};draw_particle(&mut c,&p);
  println!("{:?}",[p.x,p.y,p.vel_y,p.rotation,p.alpha() as f32,if p.is_alive(){1.0}else{0.0}]);
  println!("{:?}",c.points);
  p.update();
 }
}
}'''
with tempfile.TemporaryDirectory(prefix='arkanoo-particle-oracle-') as temp:
    source, binary = Path(temp) / 'main.rs', Path(temp) / 'oracle'
    source.write_text(body)
    subprocess.run(['rustc', '--edition=2021', '-Awarnings', str(source), '-o', str(binary)], check=True)
    rows = iter(json.loads(line) for line in subprocess.check_output([str(binary)], text=True).splitlines())
    constructor = next(rows)
    cases = []
    for _ in range(4):
        initial = next(rows)
        frames = [{'state': next(rows), 'points': next(rows)} for _ in range(int(initial[6]) + 1)]
        cases.append({'initial': initial, 'frames': frames})
print(json.dumps({'revision': '9d613e78c9ed5657ae7fda9206cd4a82a893b7c8', 'constructor': constructor, 'cases': cases}, separators=(',', ':')))
