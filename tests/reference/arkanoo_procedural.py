"""Complete original procedural background function with pinned real Rand 0.8.5."""
from pathlib import Path
import hashlib,json,os,struct,subprocess,sys,tempfile,zlib,tomllib
source=Path(sys.argv[1])/'src/rendering.rs';text=source.read_text()
assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
a=text.index('fn draw_animated_background(');b=text.index('\n#[allow(clippy::too_many_arguments)]',a);fragment=text[a:b]
header='''#![allow(non_snake_case)]
use std::io::Write;
const WINDOW_WIDTH:u32=1280;const WINDOW_HEIGHT:u32=720;
struct Window;struct Point(i32,i32);impl Point{fn new(x:i32,y:i32)->Self{Self(x,y)}}
struct Rect(i32,i32,u32,u32);impl Rect{fn new(x:i32,y:i32,w:u32,h:u32)->Self{Self(x,y,w,h)}}
struct SdlColor;impl SdlColor{fn RGBA(r:u8,g:u8,b:u8,a:u8)->u32{u32::from_be_bytes([r,g,b,a])}fn RGB(r:u8,g:u8,b:u8)->u32{Self::RGBA(r,g,b,255)}}
mod sdl2{pub mod render{pub enum BlendMode{None,Blend}}}
struct Canvas<T>{points:Vec<(i32,i32,u32)>,color:u32,background:u32,marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{fn set_blend_mode(&mut self,_:sdl2::render::BlendMode){}fn set_draw_color(&mut self,color:u32){self.color=color;}fn clear(&mut self){self.background=self.color;}
fn draw_point(&mut self,p:Point)->Result<(),()>{self.points.push((p.0,p.1,self.color));Ok(())}
fn draw_line(&mut self,_:Point,_:Point)->Result<(),()>{panic!("unexpected line theme")}
fn draw_rect(&mut self,_:Rect)->Result<(),()>{panic!("unexpected outline theme")}
fn fill_rect(&mut self,r:Rect)->Result<(),()>{for y in r.1.max(0)..(r.1+r.3 as i32).min(720){for x in r.0.max(0)..(r.0+r.2 as i32).min(1280){self.draw_point(Point(x,y))?;}}Ok(())}}
'''
body=header+fragment+'''
fn main(){let args:Vec<String>=std::env::args().collect();
if args.len()==1 {
 use rand::{Rng,SeedableRng};let mut found=[0;20];let mut levels=Vec::new();
 for level in 10u64..10000 {let seed=level.wrapping_mul(12345).wrapping_add((level/3).wrapping_mul(67890)).wrapping_add((level%7).wrapping_mul(11111));let theme=rand::rngs::StdRng::seed_from_u64(seed).gen_range(0i32..20);if [7,9,19].contains(&theme)&&found[theme as usize]<3 {levels.push((level,theme));found[theme as usize]+=1;}if [7,9,19].iter().all(|v|found[*v]==3){break;}}
 println!("{:?}",levels);return;
}
if args[1]=="half" {use rand::{Rng,SeedableRng};let seed:u64=args[2].parse().unwrap();let mut rng=rand::rngs::StdRng::seed_from_u64(seed);let samples:Vec<bool>=(0..256).map(|_|rng.gen_bool(0.5)).collect();println!("{:?}",samples);return;}
if args[1]=="rng" {use rand::{Rng,SeedableRng};let seed:u64=args[2].parse().unwrap();let mut rng=rand::rngs::StdRng::seed_from_u64(seed);let samples:Vec<u8>=(0..256).map(|i|{let(low,high)=[(0u8,255u8),(100,200),(100,255),(0,17)][i%4];rng.gen_range(low..high)}).collect();println!("{:?}",samples);return;}
let mut c=Canvas{points:Vec::new(),color:0,background:0,marker:std::marker::PhantomData};draw_animated_background(&mut c,args[1].parse().unwrap(),args[2].parse().unwrap());let mut out=std::io::BufWriter::new(std::io::stdout().lock());out.write_all(&c.background.to_le_bytes()).unwrap();for(x,y,color)in c.points{out.write_all(&x.to_le_bytes()).unwrap();out.write_all(&y.to_le_bytes()).unwrap();out.write_all(&color.to_le_bytes()).unwrap();}}
'''
c='''#include <SDL.h>
#include <stdio.h>
#include <stdint.h>
int main(void){uint8_t bg[4];if(fread(bg,1,4,stdin)!=4)return 1;SDL_Surface*s=SDL_CreateRGBSurfaceWithFormat(0,1280,720,32,SDL_PIXELFORMAT_RGBA32);if(!s)return 2;
SDL_Renderer*r=SDL_CreateSoftwareRenderer(s);if(!r)return 3;SDL_SetRenderDrawColor(r,bg[3],bg[2],bg[1],bg[0]);SDL_RenderClear(r);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);
uint8_t p[12];while(fread(p,1,12,stdin)==12){int32_t x=(int32_t)((uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24);int32_t y=(int32_t)((uint32_t)p[4]|(uint32_t)p[5]<<8|(uint32_t)p[6]<<16|(uint32_t)p[7]<<24);SDL_SetRenderDrawColor(r,p[11],p[10],p[9],p[8]);if(SDL_RenderDrawPoint(r,x,y))return 4;}
uint8_t*pixels=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,1280*4))return 5;if(fwrite(pixels,1,1280*720*4,stdout)!=1280*720*4)return 6;free(pixels);SDL_DestroyRenderer(r);SDL_FreeSurface(s);return 0;}
'''
themes=[int(value) for value in sys.argv[3].split(',')] if len(sys.argv)>3 else [7,9,19]
assert themes and len(set(themes))==len(themes) and all(0<=theme<20 for theme in themes)
assert all(theme in [1,7,9,11,16,17,18,19] for theme in themes), 'This recorder covers the adapted point/filled-rectangle themes; line and outline recording remains pending'
body=body.replace('[7,9,19]',str(themes).replace(' ',''))
rand_source=Path(sys.argv[2]);assert tomllib.loads((rand_source/'Cargo.toml').read_text())['package']['version']=='0.8.5'
with tempfile.TemporaryDirectory(prefix='gamepkg-procedural-reference-') as temporary:
 folder=Path(temporary);(folder/'src').mkdir();(folder/'src/main.rs').write_text(body);(folder/'raster.c').write_text(c)
 (folder/'Cargo.toml').write_text('[package]\nname="arkanoo-procedural-reference"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nrand={path='+json.dumps(str(rand_source))+'}\nrand_chacha="=0.3.1"\nrand_core="=0.6.4"\n')
 env=dict(os.environ,CARGO_TARGET_DIR='/tmp/gamepkg-procedural-reference-target')
 subprocess.run(['cargo','build','--release','--offline','--quiet','--manifest-path',str(folder/'Cargo.toml')],env=env,check=True)
 lock=tomllib.loads((folder/'Cargo.lock').read_text());assert {p['name']:p['version'] for p in lock['package'] if p['name'] in ['rand','rand_chacha','rand_core']}=={'rand':'0.8.5','rand_chacha':'0.3.1','rand_core':'0.6.4'}
 oracle=Path(env['CARGO_TARGET_DIR'])/'release/arkanoo-procedural-reference'
 subprocess.run(['cc',folder/'raster.c','-o',folder/'raster',*subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()],check=True)
 import ast
 levels=sorted(ast.literal_eval(subprocess.check_output([oracle],text=True)),key=lambda item:(item[1],item[0]));cases=[]
 for index,(level,theme) in enumerate(levels):
  for frame in ([0,1,59,120,16777217,4294967295,18446744073709551615] if index%3==0 else [0,120]):
   raw=subprocess.check_output([oracle,str(level),str(frame)]);background=struct.unpack('<I',raw[:4])[0];trace=raw[4:];pixels=subprocess.check_output([folder/'raster'],input=raw)
   visible=[(x,y) for x,y,color in struct.iter_unpack('<iiI',trace) if color&255 and 0<=x<1280 and 0<=y<720]
   bounds=[min(x for x,y in visible),min(y for x,y in visible),max(x for x,y in visible),max(y for x,y in visible)] if visible else None
   cases.append(dict(level=level,theme=theme,frame=frame,background=background,point_count=len(trace)//12,trace_sha256=hashlib.sha256(trace).hexdigest(),bounds=bounds,sdl_rgba_zlib_hex=zlib.compress(pixels).hex()))
 rng=[dict(seed=seed,values=json.loads(subprocess.check_output([oracle,'rng',str(seed)],text=True))) for seed in [0,1,12345,18446744073709551615]]
 half=[dict(seed=seed,values=json.loads(subprocess.check_output([oracle,'half',str(seed)],text=True))) for seed in [0,1,12345,18446744073709551615]] if 17 in themes else None
 print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),rng_u8=rng,**({'rng_half':half} if half is not None else {}),cases=cases),indent=2))
