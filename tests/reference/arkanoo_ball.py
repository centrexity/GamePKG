"""Render the unchanged pinned ball procedure through SDL's real point clipping."""
from pathlib import Path
import hashlib,json,subprocess,sys,tempfile
root=Path(sys.argv[1]);source=root/'src/rendering.rs';text=source.read_text()
start=text.index('fn draw_shiny_ball_texture(');end=text.index('\nfn draw_paddle_texture(',start)
assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83', 'Pinned rendering source changed'
entities=root/'src/entities.rs'
assert hashlib.sha256(entities.read_bytes()).hexdigest()=='ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545', 'Pinned ball dimensions/geometry source changed'
fragment=text[start:end]
with tempfile.TemporaryDirectory(prefix='arkanoo-ball-reference-') as temporary:
    work=Path(temporary)
    rust='''#![allow(non_snake_case)]
const BALL_SIZE:i32=12;
struct Window;
struct Point(i32,i32);impl Point { fn new(x:i32,y:i32)->Self {Self(x,y)} }
struct SdlColor(u8,u8,u8);impl SdlColor { fn RGB(r:u8,g:u8,b:u8)->Self {Self(r,g,b)} }
struct Canvas<T> {points:Vec<[i32;6]>,color:SdlColor,marker:std::marker::PhantomData<T>}
impl<T> Canvas<T> {
 fn set_draw_color(&mut self,color:SdlColor){self.color=color;}
 fn draw_point(&mut self,p:Point)->Result<(),()> {self.points.push([p.0,p.1,self.color.0 as i32,self.color.1 as i32,self.color.2 as i32,255]);Ok(())}
}
'''+fragment+'''
fn main(){let mut c=Canvas{points:Vec::new(),color:SdlColor(0,0,0),marker:std::marker::PhantomData::<Window>};draw_shiny_ball_texture(&mut c);for p in c.points {println!("{} {} {} {} {} {}",p[0],p[1],p[2],p[3],p[4],p[5]);}}
'''
    (work/'reference.rs').write_text(rust)
    subprocess.run(['rustc','--edition=2021','-O',str(work/'reference.rs'),'-o',str(work/'reference')],check=True)
    points=subprocess.check_output([str(work/'reference')])
    (work/'raster.c').write_text('''#include <SDL.h>
#include <stdio.h>
int main(void){
 SDL_Surface *surface=SDL_CreateRGBSurfaceWithFormat(0,12,12,32,SDL_PIXELFORMAT_RGBA32);if(!surface)return 1;
 SDL_Renderer *renderer=SDL_CreateSoftwareRenderer(surface);if(!renderer)return 2;
 SDL_SetRenderDrawColor(renderer,0,0,0,0);SDL_RenderClear(renderer);
 int x,y,r,g,b,a;while(scanf("%d %d %d %d %d %d",&x,&y,&r,&g,&b,&a)==6){SDL_SetRenderDrawColor(renderer,r,g,b,a);if(SDL_RenderDrawPoint(renderer,x,y))return 3;}
 unsigned char pixels[12*12*4];if(SDL_RenderReadPixels(renderer,NULL,SDL_PIXELFORMAT_RGBA32,pixels,12*4))return 4;
 if(fwrite(pixels,1,sizeof(pixels),stdout)!=sizeof(pixels))return 5;
 SDL_DestroyRenderer(renderer);SDL_FreeSurface(surface);return 0;
}''')
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()
    subprocess.run(['cc',str(work/'raster.c'),'-o',str(work/'raster'),*flags],check=True)
    rgba=subprocess.check_output([str(work/'raster')],input=points)
    assert len(rgba)==576
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),entities_sha256=hashlib.sha256(entities.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),rgba_sha256=hashlib.sha256(rgba).hexdigest(),width=12,height=12,rgba=list(rgba)),indent=2))
