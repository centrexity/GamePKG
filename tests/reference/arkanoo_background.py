"""Original campaign background branches, including release overflow semantics."""
from pathlib import Path
import hashlib,json,struct,subprocess,sys,tempfile,zlib
source=Path(sys.argv[1])/'src/rendering.rs';text=source.read_text()
assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
a=text.index('fn draw_animated_background(');b=text.index('        _ => {',a);fragment=text[a:b]+'        _ => {}\n    }\n}\n'
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
fn fill_rect(&mut self,r:Rect)->Result<(),()>{for y in r.1.max(0)..(r.1+r.3 as i32).min(720){for x in r.0.max(0)..(r.0+r.2 as i32).min(1280){self.draw_point(Point(x,y))?;}}Ok(())}}
'''
body=header+fragment+'''
fn main(){let args:Vec<String>=std::env::args().collect();let mut c=Canvas{points:Vec::new(),color:0,background:0,marker:std::marker::PhantomData};draw_animated_background(&mut c,args[1].parse().unwrap(),args[2].parse().unwrap());let mut out=std::io::BufWriter::new(std::io::stdout().lock());out.write_all(&c.background.to_le_bytes()).unwrap();for(x,y,color)in c.points{out.write_all(&x.to_le_bytes()).unwrap();out.write_all(&y.to_le_bytes()).unwrap();out.write_all(&color.to_le_bytes()).unwrap();}}
'''
c='''#include <SDL.h>
#include <stdio.h>
#include <stdint.h>
int main(void){uint8_t bg[4];if(fread(bg,1,4,stdin)!=4)return 1;SDL_Surface*s=SDL_CreateRGBSurfaceWithFormat(0,1280,720,32,SDL_PIXELFORMAT_RGBA32);if(!s)return 2;
SDL_Renderer*r=SDL_CreateSoftwareRenderer(s);if(!r)return 3;SDL_SetRenderDrawColor(r,bg[3],bg[2],bg[1],bg[0]);SDL_RenderClear(r);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);
uint8_t p[12];while(fread(p,1,12,stdin)==12){int32_t x=(int32_t)((uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24);int32_t y=(int32_t)((uint32_t)p[4]|(uint32_t)p[5]<<8|(uint32_t)p[6]<<16|(uint32_t)p[7]<<24);SDL_SetRenderDrawColor(r,p[11],p[10],p[9],p[8]);if(SDL_RenderDrawPoint(r,x,y))return 4;}
uint8_t*pixels=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,1280*4))return 5;if(fwrite(pixels,1,1280*720*4,stdout)!=1280*720*4)return 6;free(pixels);SDL_DestroyRenderer(r);SDL_FreeSurface(s);return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-background-reference-') as temporary:
 folder=Path(temporary);(folder/'main.rs').write_text(body);(folder/'raster.c').write_text(c)
 subprocess.run(['rustc','--edition=2021','-C','overflow-checks=no',folder/'main.rs','-o',folder/'oracle'],check=True)
 subprocess.run(['cc',folder/'raster.c','-o',folder/'raster',*subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()],check=True)
 cases=[]
 for level in [7,8,9]:
  for frame in [0,1,59,120,16777217,4294967295,18446744073709551615]:
   raw=subprocess.check_output([folder/'oracle',str(level),str(frame)]);background=struct.unpack('<I',raw[:4])[0];trace=raw[4:];pixels=subprocess.check_output([folder/'raster'],input=raw)
   cases.append(dict(level=level,frame=frame,background=background,point_count=len(trace)//12,trace_sha256=hashlib.sha256(trace).hexdigest(),trace_zlib_hex=zlib.compress(trace).hex(),sdl_rgba_zlib_hex=zlib.compress(pixels).hex()))
 print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),cases=cases),indent=2))
