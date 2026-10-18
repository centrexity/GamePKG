"""Unchanged original mixed-primitive background procedure through actual SDL renderers."""
from pathlib import Path
import hashlib,json,os,struct,subprocess,sys,tempfile,zlib,tomllib
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
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
struct Canvas<T>{commands:Vec<(u32,i32,i32,i32,i32,u32)>,color:u32,background:u32,marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{fn set_blend_mode(&mut self,_:sdl2::render::BlendMode){}fn set_draw_color(&mut self,color:u32){self.color=color;}fn clear(&mut self){self.background=self.color;}
fn draw_point(&mut self,p:Point)->Result<(),()>{self.commands.push((0,p.0,p.1,0,0,self.color));Ok(())}
fn draw_line(&mut self,a:Point,b:Point)->Result<(),()>{self.commands.push((1,a.0,a.1,b.0,b.1,self.color));Ok(())}
fn draw_rect(&mut self,r:Rect)->Result<(),()>{self.commands.push((2,r.0,r.1,r.2 as i32,r.3 as i32,self.color));Ok(())}
fn fill_rect(&mut self,r:Rect)->Result<(),()>{self.commands.push((3,r.0,r.1,r.2 as i32,r.3 as i32,self.color));Ok(())}}
'''
body=header+fragment+'''
fn main(){let args:Vec<String>=std::env::args().collect();
if args.len()==1 {
 use rand::{Rng,SeedableRng};let mut found=[0;20];let mut levels=Vec::new();
 for level in 10u64..10000 {let seed=level.wrapping_mul(12345).wrapping_add((level/3).wrapping_mul(67890)).wrapping_add((level%7).wrapping_mul(11111));let mut rng=rand::rngs::StdRng::seed_from_u64(seed);let theme=rng.gen_range(0i32..20);if [7,9,19].contains(&theme)&&found[theme as usize]<3 {if theme==8&&found[8]==0 {let mut shoot=rand::rngs::StdRng::seed_from_u64(level);if !shoot.gen_bool(0.3){continue;}}if theme==2&&found[2]==2 {let _:u8=rng.gen_range(5..40);let _:u8=rng.gen_range(5..40);let _:u8=rng.gen_range(5..40);let size:u32=rng.gen_range(20..60);if size!=20 {continue;}}levels.push((level,theme));found[theme as usize]+=1;}if [7,9,19].iter().all(|v|found[*v]==3){break;}}
 println!("{:?}",levels);return;
}
if args[1]=="probabilities" {use rand::{Rng,SeedableRng};let seed:u64=args[2].parse().unwrap();let which:usize=args[3].parse().unwrap();let mut rng=rand::rngs::StdRng::seed_from_u64(seed);let samples:Vec<bool>=(0..256).map(|_|rng.gen_bool([0.1,0.3,0.7][which])).collect();println!("{:?}",samples);return;}
if args[1]=="half" {use rand::{Rng,SeedableRng};let seed:u64=args[2].parse().unwrap();let mut rng=rand::rngs::StdRng::seed_from_u64(seed);let samples:Vec<bool>=(0..256).map(|_|rng.gen_bool(0.5)).collect();println!("{:?}",samples);return;}
if args[1]=="rng" {use rand::{Rng,SeedableRng};let seed:u64=args[2].parse().unwrap();let mut rng=rand::rngs::StdRng::seed_from_u64(seed);let samples:Vec<u8>=(0..256).map(|i|{let(low,high)=[(0u8,255u8),(100,200),(100,255),(0,17)][i%4];rng.gen_range(low..high)}).collect();println!("{:?}",samples);return;}
let mut c=Canvas{commands:Vec::new(),color:0,background:0,marker:std::marker::PhantomData};draw_animated_background(&mut c,args[1].parse().unwrap(),args[2].parse().unwrap());let mut out=std::io::BufWriter::new(std::io::stdout().lock());out.write_all(&c.background.to_le_bytes()).unwrap();for(kind,x1,y1,x2,y2,color)in c.commands{out.write_all(&kind.to_le_bytes()).unwrap();out.write_all(&x1.to_le_bytes()).unwrap();out.write_all(&y1.to_le_bytes()).unwrap();out.write_all(&x2.to_le_bytes()).unwrap();out.write_all(&y2.to_le_bytes()).unwrap();out.write_all(&color.to_le_bytes()).unwrap();}}
'''
c='''#include <SDL.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
static int32_t integer(uint8_t*p){return (int32_t)((uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24);}
int main(int argc,char**argv){if(argc!=2&&argc!=3)return 1;int layer=argc==3?atoi(argv[2]):-1;uint8_t bg[4];if(fread(bg,1,4,stdin)!=4)return 2;
SDL_Surface*s=NULL;SDL_Window*w=NULL;SDL_Renderer*r=NULL;
if(strcmp(argv[1],"software")==0){s=SDL_CreateRGBSurfaceWithFormat(0,1280,720,32,SDL_PIXELFORMAT_RGBA32);if(s)r=SDL_CreateSoftwareRenderer(s);}else{if(SDL_Init(SDL_INIT_VIDEO))return 3;w=SDL_CreateWindow("original background reference",0,0,1280,720,SDL_WINDOW_HIDDEN);if(w)r=SDL_CreateRenderer(w,-1,0);}if(!r)return 4;
SDL_RendererInfo info;if(SDL_GetRendererInfo(r,&info)||strcmp(info.name,argv[1]))return 5;
SDL_SetRenderDrawColor(r,layer>=0?0:bg[3],layer>=0?0:bg[2],layer>=0?0:bg[1],255);SDL_RenderClear(r);SDL_SetRenderDrawBlendMode(r,layer>=0?SDL_BLENDMODE_ADD:SDL_BLENDMODE_BLEND);
int index=0;uint8_t p[24];while(fread(p,1,24,stdin)==24){int selected=index++/48;if(layer>=0&&(selected!=layer||p[20]==0))continue;SDL_SetRenderDrawColor(r,layer>=0?1:p[23],layer>=0?0:p[22],layer>=0?0:p[21],layer>=0?255:p[20]);int kind=integer(p);int x=integer(p+4),y=integer(p+8),a=integer(p+12),b=integer(p+16);SDL_Rect rect={x,y,a,b};int error=kind==0?SDL_RenderDrawPoint(r,x,y):kind==1?SDL_RenderDrawLine(r,x,y,a,b):kind==2?SDL_RenderDrawRect(r,&rect):kind==3?SDL_RenderFillRect(r,&rect):-1;if(error)return 6;}
uint8_t*pixels=malloc(1280*720*4);if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,pixels,1280*4))return 7;if(fwrite(pixels,1,1280*720*4,stdout)!=1280*720*4)return 8;free(pixels);SDL_DestroyRenderer(r);if(s)SDL_FreeSurface(s);if(w)SDL_DestroyWindow(w);SDL_Quit();return 0;}
'''
themes=[int(value) for value in sys.argv[3].split(',')] if len(sys.argv)>3 else [2,10]
assert themes and len(set(themes))==len(themes) and all(value in [1,2,7,8,9,10,11,12,15,16,17,18,19] for value in themes)
body=body.replace('[7,9,19]',str(themes).replace(' ',''))
rand_source=Path(sys.argv[2]);assert tomllib.loads((rand_source/'Cargo.toml').read_text())['package']['version']=='0.8.5'
with tempfile.TemporaryDirectory(prefix='gamepkg-procedural-reference-') as temporary:
 folder=Path(temporary);(folder/'src').mkdir();(folder/'src/main.rs').write_text(body);(folder/'raster.c').write_text(c)
 (folder/'Cargo.toml').write_text('[package]\nname="arkanoo-procedural-reference"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nrand={path='+json.dumps(str(rand_source))+'}\nrand_chacha="=0.3.1"\nrand_core="=0.6.4"\n')
 env=dict(os.environ,CARGO_TARGET_DIR='/tmp/gamepkg-command-procedural-reference-target')
 subprocess.run(['cargo','build','--release','--offline','--quiet','--manifest-path',str(folder/'Cargo.toml')],env=env,check=True)
 lock=tomllib.loads((folder/'Cargo.lock').read_text());assert {p['name']:p['version'] for p in lock['package'] if p['name'] in ['rand','rand_chacha','rand_core']}=={'rand':'0.8.5','rand_chacha':'0.3.1','rand_core':'0.6.4'}
 oracle=Path(env['CARGO_TARGET_DIR'])/'release/arkanoo-procedural-reference'
 subprocess.run(['cc',folder/'raster.c','-o',folder/'raster',*subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()],check=True)
 import ast
 levels=sorted(ast.literal_eval(subprocess.check_output([oracle],text=True)),key=lambda item:(item[1],item[0]));cases=[]
 env['LIBGL_ALWAYS_SOFTWARE']='1';env.pop('WAYLAND_DISPLAY',None);env.pop('SDL_RENDER_LINE_METHOD',None)
 display=start_display(env,width=1280,height=720)
 try:
  for index,(level,theme) in enumerate(levels):
   frames=[0,1,29,30,59,60,120,199,200,201,259,260,399,400,16777217,4294967295,18446744073709551615] if theme==8 and index%3==0 else [0,1,59,120,16777217,4294967295,18446744073709551615] if index%3==0 else [0,120]
   for frame in frames:
    raw=subprocess.check_output([oracle,str(level),str(frame)]);background=struct.unpack('<I',raw[:4])[0];trace=raw[4:]
    commands=list(struct.iter_unpack('<IiiiiI',trace));images={mode:subprocess.check_output([folder/'raster',mode],input=raw,env=env) for mode in ['software','opengl']}
    pixels=images['opengl'];bg=background.to_bytes(4,'big')[:3];visible=[i for i in range(1280*720) if pixels[i*4:i*4+3]!=bg]
    bounds=[min(i%1280 for i in visible),min(i//1280 for i in visible),max(i%1280 for i in visible),max(i//1280 for i in visible)] if visible else None
    coverage={mode:{i for i in range(1280*720) if value[i*4:i*4+3]!=bg} for mode,value in images.items()}
    delta=max(abs(images['opengl'][i*4+c]-images['software'][i*4+c]) for i in range(1280*720) for c in range(3))
    counts=[subprocess.check_output([folder/'raster','opengl',str(layer)],input=raw,env=env)[::4] for layer in range((len(commands)+47)//48)]
    cases.append(dict(level=level,theme=theme,frame=frame,background=background,command_count=len(commands),command_sha256=hashlib.sha256(trace).hexdigest(),commands=commands,bounds=bounds,sdl_rgba_zlib_hex=zlib.compress(images['software']).hex(),opengl_rgba_zlib_hex=zlib.compress(images['opengl']).hex(),coverage_backend_differences=len(coverage['opengl']^coverage['software']),maximum_backend_rgb_delta=delta,opengl_layer_counts_zlib_hex=[zlib.compress(value).hex() for value in counts]))
  outlines=[]
  for rect in [[20,20,2,2],[20,20,3,3],[20,20,9,9],[20,20,18,18],[20,20,58,58],[0,0,1280,720],[-1,-1,2,2],[-2,-2,9,9],[1279,719,2,2],[1275,715,9,9],[-20,-20,9,9],[1280,720,9,9],[0,0,1,1],[20,20,1,9],[20,20,9,1],[-1,10,2,9],[10,-1,9,2],[1279,10,2,9],[10,719,9,2]]:
   raw=struct.pack('<IIiiiiI',0,2,*rect,0xc864ff96)
   masks={mode:subprocess.check_output([folder/'raster',mode,'0'],input=raw,env=env)[::4] for mode in ['software','opengl']}
   outlines.append(dict(rect=rect,**{mode+'_counts_zlib_hex':zlib.compress(value).hex() for mode,value in masks.items()}))
  probabilities=[dict(seed=seed,probability=which,values=json.loads(subprocess.check_output([oracle,'probabilities',str(seed),str(which)],text=True))) for which in range(3) for seed in [0,1,12345,18446744073709551615]] if 8 in themes or 12 in themes else None
  print(json.dumps(dict(**({'rng_probabilities':probabilities} if probabilities is not None else {}),outlines=outlines,revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),sdl_version=subprocess.check_output(['pkg-config','--modversion','sdl2'],text=True).strip(),cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
