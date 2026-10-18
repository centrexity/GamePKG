"""Trace the unchanged pinned ice branch using the original rand 0.8.5 stream."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tempfile,tomllib
parser=argparse.ArgumentParser();parser.add_argument('upstream');parser.add_argument('--rand-source',required=True);args=parser.parse_args()
source=Path(args.upstream)/'src/rendering.rs';text=source.read_text();assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
entities=Path(args.upstream)/'src/entities.rs';assert hashlib.sha256(entities.read_bytes()).hexdigest()=='ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545'
rand=Path(args.rand_source).resolve();assert tomllib.loads((rand/'Cargo.toml').read_text())['package']['version']=='0.8.5'
start=text.index('        BlockType::Ice => {',text.index('fn draw_block_with_gradient('))+len('        BlockType::Ice => {');end=text.index('\n        BlockType::Explosive => {',start);fragment=text[start:end].rsplit('},',1)[0]
RASTER_SOURCE=Path(__file__).with_name('sdl_ice_layers.c').read_text()
frames=[0,1,5,10,20,30,59,60,119,120,16777217,18446744073709551615];positions=[(40,80),(100,80),(640,260),(-1,-1)]
with tempfile.TemporaryDirectory(prefix='gamepkg-ice-reference-') as temporary:
 work=Path(temporary);(work/'src').mkdir()
 rust='''#![allow(non_snake_case)]
struct Window;struct Point(i32,i32);impl Point{fn new(x:i32,y:i32)->Self{Self(x,y)}}
#[derive(Clone,Copy)]struct Rect(i32,i32,u32,u32);impl Rect{fn new(x:i32,y:i32,w:u32,h:u32)->Self{Self(x,y,w,h)}}
struct SdlColor(u8,u8,u8,u8);impl SdlColor{fn RGBA(r:u8,g:u8,b:u8,a:u8)->Self{Self(r,g,b,a)}}
mod sdl2{pub mod render{pub enum BlendMode{None,Blend}}}
struct Canvas<T>{marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{
fn set_blend_mode(&mut self,b:sdl2::render::BlendMode){println!("B {} 0 0 0",if matches!(b,sdl2::render::BlendMode::Blend){1}else{0});}
fn set_draw_color(&mut self,c:SdlColor){println!("C {} {} {} {}",c.0,c.1,c.2,c.3);}
fn fill_rect(&mut self,r:Rect)->Result<(),()>{println!("R {} {} {} {}",r.0,r.1,r.2,r.3);Ok(())}
fn draw_line(&mut self,a:Point,b:Point)->Result<(),()>{println!("L {} {} {} {}",a.0,a.1,b.0,b.1);Ok(())}
fn set_clip_rect(&mut self,r:impl Into<Option<Rect>>){if let Some(r)=r.into(){println!("K {} {} {} {}",r.0,r.1,r.2,r.3)}else{println!("K -1 -1 0 0")}}
}
const BLOCK_WIDTH:i32=60;const BLOCK_HEIGHT:i32=20;
struct Block{x:i32,y:i32,health:i32,max_health:i32}impl Block{fn rect(&self)->Rect{Rect::new(self.x,self.y,60,20)}}
fn draw(canvas:&mut Canvas<Window>,block:&Block,frame_count:u64){
'''+fragment+'\n}\nfn main(){\n'
 for frame in frames:
  for x,y in positions:
   for health in [2,1]:
    rust+=f'println!("CASE {frame} {x} {y} {health}");draw(&mut Canvas{{marker:std::marker::PhantomData::<Window>}},&Block{{x:{x},y:{y},health:{health},max_health:2}},{frame});\n'
 rust+='}'
 (work/'src/main.rs').write_text(rust);(work/'Cargo.toml').write_text('[package]\nname="arkanoo-ice-reference"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nrand={path='+json.dumps(str(rand))+'}\nrand_chacha="=0.3.1"\nrand_core="=0.6.4"\n')
 output=subprocess.check_output(['cargo','run','--offline','--quiet','--release','--manifest-path',str(work/'Cargo.toml')],env=dict(os.environ,CARGO_TARGET_DIR='/tmp/gamepkg-arkanoo-ice-reference-target'),text=True)
 lock=tomllib.loads((work/'Cargo.lock').read_text());assert next(p['version'] for p in lock['package'] if p['name']=='rand_chacha')=='0.3.1'
 cases=[]
 for line in output.splitlines():
  row=line.split()
  if row[0]=='CASE':
   frame,x,y,health=map(int,row[1:]);cases.append(dict(frame=frame,x=x,y=y,health=health,commands=[]))
  else:cases[-1]['commands'].append([row[0],*map(int,row[1:])])
 # Rasterize the original line calls through SDL, independently of guest code.
 (work/'raster.c').write_text(RASTER_SOURCE)
 flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split();subprocess.run(['cc',str(work/'raster.c'),'-o',str(work/'raster'),*flags],check=True)
 masks={}
 for case in cases:
  commands=''.join(' '.join(map(str,row))+'\n' for row in case['commands']).encode()
  paths=[]
  for mode,name in [(1,'overlay'),(2,'shine')]:
   rgba=subprocess.check_output([str(work/'raster'),str(mode),str(case['x']),str(case['y']),'00000000'],input=commands)
   layer=work/(name+'.rgba');layer.write_bytes(rgba);paths.append(str(layer))
   alpha=rgba[3::4];digest=hashlib.sha256(alpha).hexdigest();masks[digest]=alpha.hex();case[name]=digest
  case['composites']=[]
  for background in ['000000ff','ffffffff','0b1020ff','505064ff','00000000','12345678']:
   original=subprocess.check_output([str(work/'raster'),'0',str(case['x']),str(case['y']),background],input=commands)
   grouped=subprocess.check_output([str(work/'raster'),'4',str(case['x']),str(case['y']),background,*paths],input=commands)
   delta=max(abs(a-b) for a,b in zip(original,grouped))
   assert delta<=3,(case['frame'],case['x'],case['y'],case['health'],background,delta)
   case['composites'].append(dict(background=background,max_channel_delta=delta,original_sha256=hashlib.sha256(original).hexdigest(),grouped_sha256=hashlib.sha256(grouped).hexdigest()))
 print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),rand_version='0.8.5',masks=masks,cases=cases),indent=2))
