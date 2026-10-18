"""Capture the complete unchanged portal procedure and rasterize through real SDL."""
from pathlib import Path
import hashlib, json, struct, subprocess, sys, tempfile, zlib
root=Path(sys.argv[1]);source=root/'src/rendering.rs';text=source.read_text()
assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
start=text.index('fn draw_portal(');fragment=text[start:text.index('\nfn render_hud(',start)]
header='''#![allow(non_snake_case)]
use std::io::Write;
const WINDOW_WIDTH:u32=1280;const WINDOW_HEIGHT:u32=720;
struct Window;struct Point(i32,i32);impl Point{fn new(x:i32,y:i32)->Self{Self(x,y)}}
struct SdlColor;impl SdlColor{fn RGBA(r:u8,g:u8,b:u8,a:u8)->u32{u32::from_be_bytes([r,g,b,a])}}
mod sdl2{pub mod render{pub enum BlendMode{None,Blend}}}
struct Canvas<T>{points:Vec<(i32,i32,u32)>,color:u32,marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{fn set_blend_mode(&mut self,_:sdl2::render::BlendMode){}fn set_draw_color(&mut self,color:u32){self.color=color;}
fn draw_point(&mut self,p:Point)->Result<(),()>{self.points.push((p.0,p.1,self.color));Ok(())}}
'''
body=header+fragment+'''
fn main(){let args:Vec<String>=std::env::args().collect();let mut c=Canvas{points:Vec::new(),color:0,marker:std::marker::PhantomData};draw_portal(&mut c,args[1].parse().unwrap(),args[2].parse().unwrap());let mut out=std::io::BufWriter::new(std::io::stdout().lock());for(x,y,color)in c.points{out.write_all(&x.to_le_bytes()).unwrap();out.write_all(&y.to_le_bytes()).unwrap();out.write_all(&color.to_le_bytes()).unwrap();}}
'''
c='''#include <SDL.h>
#include <stdio.h>
#include <stdint.h>
int main(void){SDL_Surface*s=SDL_CreateRGBSurfaceWithFormat(0,1280,720,32,SDL_PIXELFORMAT_RGBA32);if(!s)return 1;
SDL_Renderer*r=SDL_CreateSoftwareRenderer(s);if(!r)return 2;SDL_SetRenderDrawColor(r,11,16,32,255);SDL_RenderClear(r);SDL_SetRenderDrawBlendMode(r,SDL_BLENDMODE_BLEND);
uint8_t p[12];while(fread(p,1,12,stdin)==12){int32_t x=(int32_t)((uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24);int32_t y=(int32_t)((uint32_t)p[4]|(uint32_t)p[5]<<8|(uint32_t)p[6]<<16|(uint32_t)p[7]<<24);SDL_SetRenderDrawColor(r,p[11],p[10],p[9],p[8]);if(SDL_RenderDrawPoint(r,x,y))return 3;}
uint8_t*pixels=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,1280*4))return 4;if(fwrite(pixels,1,1280*720*4,stdout)!=1280*720*4)return 5;free(pixels);SDL_DestroyRenderer(r);SDL_FreeSurface(s);return 0;}
'''
frames=[(0,0),(1,1),(20,30),(30,31),(40,90),(149,149),(150,150),(151,151),(166,166),(180,180),(181,181),(225,225),(269,269),(270,270),(271,271),(16777217,0),(18446744073709551615,31),(18446744073709551615,18446744073709551615)]
with tempfile.TemporaryDirectory(prefix='gamepkg-portal-reference-') as temporary:
    folder=Path(temporary);(folder/'main.rs').write_text(body);(folder/'raster.c').write_text(c)
    subprocess.run(['rustc','--edition=2021',folder/'main.rs','-o',folder/'oracle'],check=True)
    subprocess.run(['cc',folder/'raster.c','-o',folder/'raster',*subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()],check=True)
    cases=[]
    for frame,timer in frames:
        trace=subprocess.check_output([folder/'oracle',str(frame),str(timer)])
        dots=list(struct.iter_unpack('<iiI',trace));visible=[(x,y) for x,y,color in dots if color&255]
        bounds=None;rgba=b''
        if visible:
            left=min(x for x,y in visible);top=min(y for x,y in visible);width=max(x for x,y in visible)-left+1;height=max(y for x,y in visible)-top+1
            bounds=[left,top,width,height];full=subprocess.check_output([folder/'raster'],input=trace)
            rgba=b''.join(full[(y*1280+left)*4:(y*1280+left+width)*4] for y in range(top,top+height))
        cases.append(dict(frame=frame,timer=timer,point_count=len(dots),trace_sha256=hashlib.sha256(trace).hexdigest(),trace_zlib_hex=zlib.compress(trace).hex(),bounds=bounds,sdl_dark_rgba_zlib_hex=zlib.compress(rgba).hex()))
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),cases=cases),indent=2))
