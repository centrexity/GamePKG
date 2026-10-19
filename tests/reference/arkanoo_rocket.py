"""Maintainer oracle: unchanged original rocket primitives, rasterized by real SDL."""
from pathlib import Path
import argparse, hashlib, json, struct, subprocess, tempfile, zlib
parser = argparse.ArgumentParser()
parser.add_argument('original', type=Path)
parser.add_argument('--write-asset', type=Path)
args = parser.parse_args()
source = args.original / 'src/rendering.rs'
assert hashlib.sha256(source.read_bytes()).hexdigest() == '51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
text = source.read_text()
fragment = text[text.index('    // Draw rocket projectiles'):text.index('    // Draw particles', text.index('    // Draw rocket projectiles'))]
header = '''#![allow(non_snake_case)]
struct Rect(i32,i32,u32,u32);impl Rect{fn new(x:i32,y:i32,w:u32,h:u32)->Self{Self(x,y,w,h)}}
struct Point(i32,i32);impl Point{fn new(x:i32,y:i32)->Self{Self(x,y)}}
struct SdlColor(u8,u8,u8);impl SdlColor{fn RGB(r:u8,g:u8,b:u8)->Self{Self(r,g,b)}}
struct Canvas;impl Canvas{
fn set_draw_color(&mut self,c:SdlColor){println!("C {} {} {} 255",c.0,c.1,c.2);}
fn fill_rect(&mut self,r:Rect)->Result<(),()>{println!("R {} {} {} {}",r.0,r.1,r.2,r.3);Ok(())}
fn draw_line(&mut self,a:Point,b:Point)->Result<(),()>{println!("L {} {} {} {}",a.0,a.1,b.0,b.1);Ok(())}}
struct Rocket{x:f32,y:f32,active:bool}struct Game{rockets:Vec<Rocket>}
fn main(){let a:Vec<String>=std::env::args().collect();let game=Game{rockets:vec![Rocket{x:a[1].parse().unwrap(),y:a[2].parse().unwrap(),active:a[3]=="1"}]};let mut canvas=Canvas;
'''
c = '''#include <SDL.h>
#include <stdio.h>
int main(void){SDL_Surface*s=SDL_CreateRGBSurfaceWithFormat(0,10,20,32,SDL_PIXELFORMAT_RGBA32);if(!s)return 1;
SDL_Renderer*r=SDL_CreateSoftwareRenderer(s);if(!r)return 2;SDL_SetRenderDrawColor(r,0,0,0,0);SDL_RenderClear(r);
char op;int x,y,w,h;while(scanf(" %c %d %d %d %d",&op,&x,&y,&w,&h)==5){int e=0;
if(op=='C')e=SDL_SetRenderDrawColor(r,x,y,w,h);else if(op=='R'){SDL_Rect q={x,y,w,h};e=SDL_RenderFillRect(r,&q);}else if(op=='L')e=SDL_RenderDrawLine(r,x,y,w,h);else return 3;if(e)return 4;}
unsigned char p[800];if(SDL_RenderReadPixels(r,NULL,SDL_PIXELFORMAT_RGBA32,p,40))return 5;
if(fwrite(p,1,800,stdout)!=800)return 6;SDL_DestroyRenderer(r);SDL_FreeSurface(s);return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-rocket-reference-') as tmp:
    folder = Path(tmp)
    (folder/'main.rs').write_text(header + fragment + '}')
    (folder/'raster.c').write_text(c)
    subprocess.run(['rustc', '--edition=2021', folder/'main.rs', '-o', folder/'oracle'], check=True)
    subprocess.run(['cc', folder/'raster.c', '-o', folder/'raster', *subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()], check=True)
    canonical = subprocess.check_output([folder/'oracle','0','0','1'])
    pixels = subprocess.check_output([folder/'raster'], input=canonical)
    cases = []
    for x,y in [(0,0),(320.75,240.25),(-.75,-.75),(-1.75,-1.75),(1279.5,719.75)]:
        commands = subprocess.check_output([folder/'oracle',str(x),str(y),'1'],text=True)
        normalized = []
        for line in commands.splitlines():
            kind,*values = line.split(); values = list(map(int,values))
            if kind in ['R','L']:
                values[0] -= int(x); values[1] -= int(y)
                if kind == 'L': values[2] -= int(x); values[3] -= int(y)
            normalized.append(kind+' '+' '.join(map(str,values)))
        assert '\n'.join(normalized)+'\n' == canonical.decode()
        cases.append(dict(source=[x,y],origin=[int(x),int(y)],trace_sha256=hashlib.sha256(commands.encode()).hexdigest()))
    assert not subprocess.check_output([folder/'oracle','0','0','0'])
    if args.write_asset:
        def chunk(kind,data): return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data))
        scanlines = b''.join(b'\0'+pixels[y*40:(y+1)*40] for y in range(20))
        args.write_asset.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',10,20,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(scanlines))+chunk(b'IEND',b''))
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),width=10,height=20,rgba_hex=pixels.hex(),rgba_sha256=hashlib.sha256(pixels).hexdigest(),primitive_count=13,cases=cases),indent=2))
