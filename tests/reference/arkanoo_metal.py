"""Extract unchanged graphite drawing and measure its three layers with real SDL."""
from pathlib import Path
import hashlib,json,subprocess,sys,tempfile
root=Path(sys.argv[1]);source=root/'src/rendering.rs';entities=root/'src/entities.rs'
assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
assert hashlib.sha256(entities.read_bytes()).hexdigest()=='ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545'
text=source.read_text();start=text.index('        BlockType::Undestroyable => {',text.index('fn draw_block_with_gradient('))+len('        BlockType::Undestroyable => {')
end=text.index('\n        BlockType::Normal => {',start);fragment=text[start:end].rsplit('},',1)[0]
with tempfile.TemporaryDirectory(prefix='arkanoo-metal-reference-') as temporary:
    work=Path(temporary)
    rust='''#![allow(non_snake_case)]
struct Window;struct Point(i32,i32);impl Point{fn new(x:i32,y:i32)->Self{Self(x,y)}}
struct Rect(i32,i32,u32,u32);impl Rect{fn new(x:i32,y:i32,w:u32,h:u32)->Self{Self(x,y,w,h)}}
struct SdlColor(u8,u8,u8,u8);impl SdlColor{fn RGB(r:u8,g:u8,b:u8)->Self{Self(r,g,b,255)} fn RGBA(r:u8,g:u8,b:u8,a:u8)->Self{Self(r,g,b,a)}}
mod sdl2{pub mod render{pub enum BlendMode{None,Blend}}}
struct Canvas<T>{marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{
 fn set_draw_color(&mut self,c:SdlColor){println!("C {} {} {} {}",c.0,c.1,c.2,c.3);}
 fn set_blend_mode(&mut self,b:sdl2::render::BlendMode){println!("B {} 0 0 0",if matches!(b,sdl2::render::BlendMode::Blend){1}else{0});}
 fn fill_rect(&mut self,r:Rect)->Result<(),()>{println!("R {} {} {} {}",r.0,r.1,r.2,r.3);Ok(())}
 fn draw_line(&mut self,a:Point,b:Point)->Result<(),()>{println!("L {} {} {} {}",a.0,a.1,b.0,b.1);Ok(())}
}
const BLOCK_WIDTH:i32=60;const BLOCK_HEIGHT:i32=20;
struct Block{x:i32,y:i32}impl Block{fn rect(&self)->Rect{Rect::new(self.x,self.y,60,20)}}
fn draw(canvas:&mut Canvas<Window>,block:&Block){
'''+fragment+'''
}
fn main(){draw(&mut Canvas{marker:std::marker::PhantomData::<Window>},&Block{x:0,y:0});}
'''
    (work/'reference.rs').write_text(rust);subprocess.run(['rustc','--edition=2021','-O',str(work/'reference.rs'),'-o',str(work/'reference')],check=True)
    commands=subprocess.check_output([str(work/'reference')])
    (work/'raster.c').write_text('''#include <SDL.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char **argv){
 if(argc<3)return 1;int mode=atoi(argv[1]),w=mode==1?60:91,h=mode==1?20:21,ox=mode==1?0:20;
 unsigned long bg=strtoul(argv[2],NULL,16);
 SDL_Surface *surface=SDL_CreateRGBSurfaceWithFormat(0,w,h,32,SDL_PIXELFORMAT_RGBA32);if(!surface)return 2;
 SDL_Renderer *renderer=SDL_CreateSoftwareRenderer(surface);if(!renderer)return 3;
 SDL_SetRenderDrawColor(renderer,bg>>24,bg>>16,bg>>8,bg);SDL_RenderClear(renderer);
 if(mode==4){
  if(argc!=6)return 4;
  for(int i=0;i<3;i++){
   int tw=i==0?60:91,th=i==0?20:21,n=tw*th*4;unsigned char data[n];FILE*f=fopen(argv[i+3],"rb");if(!f||fread(data,1,n,f)!=(size_t)n)return 5;fclose(f);
   SDL_Surface*s=SDL_CreateRGBSurfaceWithFormatFrom(data,tw,th,32,tw*4,SDL_PIXELFORMAT_RGBA32);if(!s)return 6;
   SDL_Texture*t=SDL_CreateTextureFromSurface(renderer,s);if(!t)return 7;SDL_SetTextureBlendMode(t,SDL_BLENDMODE_BLEND);
   SDL_Rect dst={i==0?20:0,0,tw,th};if(SDL_RenderCopy(renderer,t,NULL,&dst))return 8;
   SDL_DestroyTexture(t);SDL_FreeSurface(s);
  }
 }else{
  char op;int x,y,z,t,r=0,g=0,b=0,a=0;
  while(scanf(" %c %d %d %d %d",&op,&x,&y,&z,&t)==5){
   int result=0;switch(op){
   case 'C':r=x;g=y;b=z;a=t;result=SDL_SetRenderDrawColor(renderer,r,g,b,a);break;
   case 'B':result=SDL_SetRenderDrawBlendMode(renderer,mode>=2?SDL_BLENDMODE_NONE:(x?SDL_BLENDMODE_BLEND:SDL_BLENDMODE_NONE));break;
   case 'R':if(mode<2){SDL_Rect rect={x+ox,y,z,t};result=SDL_RenderFillRect(renderer,&rect);}break;
   case 'L':if(mode<2 || (mode==2&&r==255&&g==255&&b==255&&a==30) || (mode==3&&((r==200&&g==200&&b==210&&a==100)||(r==0&&g==0&&b==0&&a==150))))result=SDL_RenderDrawLine(renderer,x+ox,y,z+ox,t);break;
   default:return 9;}if(result)return 10;
  }
 }
 unsigned char pixels[w*h*4];if(SDL_RenderReadPixels(renderer,NULL,SDL_PIXELFORMAT_RGBA32,pixels,w*4))return 11;
 if(mode==2||mode==3){for(int y=0;y<20;y++)for(int x=20;x<80;x++)for(int c=0;c<4;c++)pixels[(y*w+x)*4+c]=0;}
 if(fwrite(pixels,1,sizeof(pixels),stdout)!=sizeof(pixels))return 12;
 SDL_DestroyRenderer(renderer);SDL_FreeSurface(surface);return 0;
}''')
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split();subprocess.run(['cc',str(work/'raster.c'),'-o',str(work/'raster'),*flags],check=True)
    layers=[];paths=[]
    for mode,name,width,height in [(1,'metal-body.png',60,20),(2,'metal-stripes.png',91,21),(3,'metal-bevel.png',91,21)]:
        rgba=subprocess.check_output([str(work/'raster'),str(mode),'00000000'],input=commands);assert len(rgba)==width*height*4
        path=work/name;path.write_bytes(rgba);paths.append(str(path))
        layers.append(dict(name=name,width=width,height=height,rgba_sha256=hashlib.sha256(rgba).hexdigest(),rgba_hex=rgba.hex()))
    composites=[]
    for background in ['000000ff','ffffffff','0b1020ff','505064ff','00000000','12345678']:
        original=subprocess.check_output([str(work/'raster'),'0',background],input=commands)
        composed=subprocess.check_output([str(work/'raster'),'4',background,*paths])
        delta=max(abs(a-b) for a,b in zip(original,composed))
        # SDL's software line and texture-copy paths round alpha differently.
        # Keep that difference measured, rather than claiming exact composition.
        assert delta<=2,(background,delta)
        for y in range(20):
            for x in range(20,80):
                offset=(y*91+x)*4
                assert original[offset:offset+4]==composed[offset:offset+4]
        composites.append(dict(background=background,original_rgba_sha256=hashlib.sha256(original).hexdigest(),layered_rgba_sha256=hashlib.sha256(composed).hexdigest(),max_channel_delta=delta,differing_bytes=sum(a!=b for a,b in zip(original,composed))))
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),entities_sha256=hashlib.sha256(entities.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),layers=layers,composites=composites),indent=2))
