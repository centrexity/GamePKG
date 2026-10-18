"""Trace and rasterize the unchanged pinned explosive-brick branch."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tempfile,tarfile,zlib
parser=argparse.ArgumentParser();parser.add_argument('upstream');parser.add_argument('--sdl-crate',required=True);parser.add_argument('--sdl-sys-crate',required=True);args=parser.parse_args()
source=Path(args.upstream)/'src/rendering.rs';text=source.read_text();assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
entities=Path(args.upstream)/'src/entities.rs';assert hashlib.sha256(entities.read_bytes()).hexdigest()=='ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545'

start=text.index('        BlockType::Explosive => {',text.index('fn draw_block_with_gradient('))+len('        BlockType::Explosive => {');end=text.index('\n        BlockType::Undestroyable => {',start);fragment=text[start:end].rsplit('},',1)[0]
RASTER_SOURCE=Path(__file__).with_name('sdl_fire_layers.c').read_text()
frames=[0,1,5,10,20,30,40,60,79,80,119,120,126,16777217,9007199254740993,18446744073709551615];positions=[(40,80),(100,80),(640,260),(-1,-1)]
with tempfile.TemporaryDirectory(prefix='gamepkg-fire-reference-') as temporary:
 work=Path(temporary);(work/'src').mkdir()
 for archive,digest in [(args.sdl_crate,'3b498da7d14d1ad6c839729bd4ad6fc11d90a57583605f3b4df2cd709a9cd380'),(args.sdl_sys_crate,'951deab27af08ed9c6068b7b0d05a93c91f0a8eb16b6b816a5e73452a43521d3')]:
  assert hashlib.sha256(Path(archive).read_bytes()).hexdigest()==digest,'Original SDL dependency checksum mismatch'
  with tarfile.open(archive) as bundle:bundle.extractall(work,filter='data')
 rust='''#![allow(dead_code)]
use sdl2::{rect::{Rect,Point},pixels::Color as SdlColor,video::Window};
struct Canvas<T>{marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{
fn set_blend_mode(&mut self,b:sdl2::render::BlendMode){println!("B {} 0 0 0",if matches!(b,sdl2::render::BlendMode::Blend){1}else{0});}
fn set_draw_color(&mut self,c:SdlColor){println!("C {} {} {} {}",c.r,c.g,c.b,c.a);}
fn fill_rect(&mut self,r:Rect)->Result<(),()>{println!("R {} {} {} {}",r.x(),r.y(),r.width(),r.height());Ok(())}
fn draw_line(&mut self,a:Point,b:Point)->Result<(),()>{println!("L {} {} {} {}",a.x(),a.y(),b.x(),b.y());Ok(())}
fn set_clip_rect(&mut self,r:impl Into<Option<Rect>>){if let Some(r)=r.into(){println!("K {} {} {} {}",r.x(),r.y(),r.width(),r.height())}else{println!("K -1 -1 0 0")}}
}
const BLOCK_WIDTH:i32=60;const BLOCK_HEIGHT:i32=20;
struct Block{x:i32,y:i32,health:i32,max_health:i32}impl Block{fn rect(&self)->Rect{Rect::new(self.x,self.y,60,20)}}
fn draw(canvas:&mut Canvas<Window>,block:&Block,frame_count:u64){
'''+fragment+'\n}\nfn main(){\n'
 for frame in frames:
  for x,y in positions:
   for health in [1]:
    rust+=f'println!("CASE {frame} {x} {y} {health}");draw(&mut Canvas{{marker:std::marker::PhantomData::<Window>}},&Block{{x:{x},y:{y},health:{health},max_health:2}},{frame});\n'
 rust+='}'
 (work/'src/main.rs').write_text(rust)
 (work/'Cargo.toml').write_text('[package]\nname="arkanoo-fire-reference"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nsdl2={path='+json.dumps(str(work/'sdl2-0.37.0'))+'}\n[patch.crates-io]\nsdl2-sys={path='+json.dumps(str(work/'sdl2-sys-0.37.0'))+'}\n')
 output=subprocess.check_output(['cargo','run','--offline','--quiet','--release','--manifest-path',str(work/'Cargo.toml')],env=dict(os.environ,CARGO_TARGET_DIR='/tmp/gamepkg-fire-sdl-reference-target'),text=True)
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
   without_blob=subprocess.check_output([str(work/'raster'),'5',str(case['x']),str(case['y']),background],input=commands)
   assert original!=without_blob,'Moving one-pixel blob was not observed'
   grouped=subprocess.check_output([str(work/'raster'),'4',str(case['x']),str(case['y']),background,*paths],input=commands)
   delta=max(abs(a-b) for a,b in zip(original,grouped))
   assert delta<=3,(case['frame'],case['x'],case['y'],case['health'],background,delta)
   case['composites'].append(dict(background=background,max_channel_delta=delta,original_sha256=hashlib.sha256(original).hexdigest(),original_rgba_zlib_hex=zlib.compress(original,9).hex(),grouped_sha256=hashlib.sha256(grouped).hexdigest(),blob_changed_bytes=sum(a!=b for a,b in zip(original,without_blob))))
 print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),sdl2_version='0.37.0',sdl2_sys_crate_sha256='951deab27af08ed9c6068b7b0d05a93c91f0a8eb16b6b816a5e73452a43521d3',sdl2_crate_sha256='3b498da7d14d1ad6c839729bd4ad6fc11d90a57583605f3b4df2cd709a9cd380',masks=masks,cases=cases),indent=2))
