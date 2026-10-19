"""Render the unchanged pinned normal-brick procedure through SDL's real blending, lines and clipping."""
from pathlib import Path
import hashlib,json,subprocess,sys,tempfile
root=Path(sys.argv[1]);source=root/'src/rendering.rs';text=source.read_text()
start=text.index('fn draw_block_texture(');end=text.index('\n\n/// Draw a shiny metal ball',start)
assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83', 'Pinned rendering source changed'
entities=root/'src/entities.rs'
assert hashlib.sha256(entities.read_bytes()).hexdigest()=='ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545', 'Pinned brick dimensions/colors source changed'
fragment=text[start:end]
colors_source=entities.read_text()
colors_source=colors_source[colors_source.index('pub const BLOCK_COLORS:'):]
colors_source=colors_source[:colors_source.index('];')+2]
with tempfile.TemporaryDirectory(prefix='arkanoo-block-reference-') as temporary:
    work=Path(temporary)
    rust='''#![allow(non_snake_case,dead_code)]
struct Window;
struct Point(i32,i32);impl Point {fn new(x:i32,y:i32)->Self{Self(x,y)}}
#[derive(Clone,Copy)]
struct Rect(i32,i32,u32,u32);impl Rect {fn new(x:i32,y:i32,w:u32,h:u32)->Self{Self(x,y,w,h)} fn x(&self)->i32{self.0} fn y(&self)->i32{self.1} fn width(&self)->u32{self.2}}
struct SdlColor(u8,u8,u8,u8);impl SdlColor {fn RGB(r:u8,g:u8,b:u8)->Self{Self(r,g,b,255)} fn RGBA(r:u8,g:u8,b:u8,a:u8)->Self{Self(r,g,b,a)}}
mod sdl2 {pub mod render {pub enum BlendMode {None,Blend}}}
struct Canvas<T>{marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{
 fn set_draw_color(&mut self,c:SdlColor){println!("C {} {} {} {}",c.0,c.1,c.2,c.3);}
 fn set_blend_mode(&mut self,b:sdl2::render::BlendMode){println!("B {} 0 0 0",if matches!(b,sdl2::render::BlendMode::Blend){1}else{0});}
 fn fill_rect(&mut self,r:Rect)->Result<(),()>{println!("R {} {} {} {}",r.0,r.1,r.2,r.3);Ok(())}
 fn copy(&mut self,_:&u8,_:Option<Rect>,_:Option<Rect>)->Result<(),()>{Ok(())}
 fn draw_rect(&mut self,r:Rect)->Result<(),()>{println!("O {} {} {} {}",r.0,r.1,r.2,r.3);Ok(())}
 fn draw_point(&mut self,p:Point)->Result<(),()>{println!("P {} {} 0 0",p.0,p.1);Ok(())}
 fn draw_line(&mut self,a:Point,b:Point)->Result<(),()>{println!("L {} {} {} {}",a.0,a.1,b.0,b.1);Ok(())}
}
const BLOCK_WIDTH:i32=60;const BLOCK_HEIGHT:i32=20;
pub struct Color{r:u8,g:u8,b:u8}
impl Color{const fn new(r:u8,g:u8,b:u8)->Self{Self{r,g,b}}}
'''+colors_source+fragment+'''
fn main(){let index:usize=std::env::args().nth(1).unwrap().parse().unwrap();let mut c=Canvas{marker:std::marker::PhantomData::<Window>};let color=&BLOCK_COLORS[index];draw_block_texture(&mut c,Color::new(color.r,color.g,color.b));}
'''
    (work/'reference.rs').write_text(rust)
    subprocess.run(['rustc','--edition=2021','-O',str(work/'reference.rs'),'-o',str(work/'reference')],check=True)
    (work/'raster.c').write_text('''#include <SDL.h>
#include <stdio.h>
int main(int argc,char **argv){
 if(argc!=2)return 6;int w=atoi(argv[1]);
 SDL_Surface *surface=SDL_CreateRGBSurfaceWithFormat(0,w,20,32,SDL_PIXELFORMAT_RGBA32);if(!surface)return 1;
 SDL_Renderer *renderer=SDL_CreateSoftwareRenderer(surface);if(!renderer)return 2;
 SDL_SetRenderDrawColor(renderer,0,0,0,0);SDL_RenderClear(renderer);
 char op;int x,y,z,t;while(scanf(" %c %d %d %d %d",&op,&x,&y,&z,&t)==5){
 int result=0;switch(op){
 case 'C':result=SDL_SetRenderDrawColor(renderer,x,y,z,t);break;
 case 'B':result=SDL_SetRenderDrawBlendMode(renderer,x?SDL_BLENDMODE_BLEND:SDL_BLENDMODE_NONE);break;
 case 'P':result=SDL_RenderDrawPoint(renderer,x,y);break;
 case 'L':result=SDL_RenderDrawLine(renderer,x,y,z,t);break;
 case 'R':{SDL_Rect r={x,y,z,t};result=SDL_RenderFillRect(renderer,&r);break;}
 case 'O':{SDL_Rect r={x,y,z,t};result=SDL_RenderDrawRect(renderer,&r);break;}
 default:return 7;}if(result)return 3;}
 unsigned char pixels[w*20*4];if(SDL_RenderReadPixels(renderer,NULL,SDL_PIXELFORMAT_RGBA32,pixels,w*4))return 4;
 if(fwrite(pixels,1,sizeof(pixels),stdout)!=sizeof(pixels))return 5;
 SDL_DestroyRenderer(renderer);SDL_FreeSurface(surface);return 0;
}''')
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()
    subprocess.run(['cc',str(work/'raster.c'),'-o',str(work/'raster'),*flags],check=True)
    images=[]
    for index in range(6):
        commands=subprocess.check_output([str(work/'reference'),str(index)])
        rgba=subprocess.check_output([str(work/'raster'),'60'],input=commands)
        assert len(rgba)==60*20*4
        images.append(dict(color_index=index,width=60,height=20,rgba_sha256=hashlib.sha256(rgba).hexdigest(),rgba_hex=rgba.hex()))
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),entities_sha256=hashlib.sha256(entities.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),images=images),indent=2))
