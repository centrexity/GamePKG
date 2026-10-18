"""Extract unchanged bonus art with real pinned SDL constructors and SDL rasterization."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tempfile,tarfile,sys
parser=argparse.ArgumentParser();parser.add_argument('upstream');parser.add_argument('--sdl-crate',required=True);parser.add_argument('--sdl-sys-crate',required=True);parser.add_argument('--write-assets');parser.add_argument('--commands-only',action='store_true');args=parser.parse_args()
sys.path.insert(0,str(Path(__file__).resolve().parents[1]));from gui_test_support import start_display
source=Path(args.upstream)/'src/rendering.rs';text=source.read_text();assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
entities=Path(args.upstream)/'src/entities.rs';assert hashlib.sha256(entities.read_bytes()).hexdigest()=='ee6fc52f2ea582900edee90350c0c4608235aeecae6e56cc875cfd7893313545'
fragment=text[text.index('fn draw_bonus_icon('):text.index('\n/// Draw animated background',text.index('fn draw_bonus_icon('))]
entity=entities.read_text();kind=entity[entity.index('#[derive(Clone, Copy, PartialEq)]\npub enum BonusType'):entity.index('\n\n',entity.index('pub enum BonusType'))]
a=entity.index('    pub fn rect(',entity.index('impl Bonus {'));b=entity.index('\n    }',a)+6;rect=entity[a:b]
class Display:
 def __enter__(self):
  self.env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');self.env.pop('WAYLAND_DISPLAY',None);self.env.pop('GAMEPKG_BONUS_SOFTWARE',None);self.process=start_display(self.env,width=640,height=480);return self.env
 def __exit__(self,*_):self.process.terminate();self.process.wait(timeout=10)
with Display() as render_env,tempfile.TemporaryDirectory(prefix='gamepkg-bonus-reference-') as temporary:
 work=Path(temporary);(work/'src').mkdir()
 for archive,digest in [(args.sdl_crate,'3b498da7d14d1ad6c839729bd4ad6fc11d90a57583605f3b4df2cd709a9cd380'),(args.sdl_sys_crate,'951deab27af08ed9c6068b7b0d05a93c91f0a8eb16b6b816a5e73452a43521d3')]:
  assert hashlib.sha256(Path(archive).read_bytes()).hexdigest()==digest
  with tarfile.open(archive) as bundle:bundle.extractall(work,filter='data')
 rust='''#![allow(dead_code)]
use sdl2::{rect::{Rect,Point},pixels::Color as SdlColor,video::Window};
struct Canvas<T>{count:usize,color:SdlColor,marker:std::marker::PhantomData<T>}
impl<T> Canvas<T>{
fn set_blend_mode(&mut self,_:sdl2::render::BlendMode){}
fn set_draw_color(&mut self,c:SdlColor){self.color=c;}
fn layer(&self)->usize{if self.count<1257{0}else if self.count<1574{1}else if self.count<2294{2}else{3}}
fn draw_point(&mut self,p:Point)->Result<(),()>{println!("P {} {} {} 0 0 {} {} {} {}",self.layer(),p.x(),p.y(),self.color.r,self.color.g,self.color.b,self.color.a);self.count+=1;Ok(())}
fn fill_rect(&mut self,r:Rect)->Result<(),()>{println!("R {} {} {} {} {} {} {} {} {}",self.layer(),r.x(),r.y(),r.width(),r.height(),self.color.r,self.color.g,self.color.b,self.color.a);Ok(())}
}
'''+kind+'''
struct Bonus{x:f32,y:f32,bonus_type:BonusType}impl Bonus{
'''+rect+'\n}\n'+fragment+'\nfn main(){\n'
 for index,name in enumerate(['LongPaddle','ExtraBall','GhostBall','Rocket']):
  for position,(x,y) in enumerate([(0.0,0.0),(320.75,240.25),(-0.75,-0.75),(-1.75,-1.75)]):
   rust+=f'let bonus=Bonus{{x:{x}f32,y:{y}f32,bonus_type:BonusType::{name}}};let rect=bonus.rect();println!("CASE {index} {position} {{}} {{}}",rect.x(),rect.y());draw_bonus_icon(&mut Canvas{{count:0,color:SdlColor::RGBA(0,0,0,0),marker:std::marker::PhantomData::<Window>}},&bonus);\n'
 rust+='}'
 (work/'src/main.rs').write_text(rust);(work/'Cargo.toml').write_text('[package]\nname="arkanoo-bonus-reference"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nsdl2={path='+json.dumps(str(work/'sdl2-0.37.0'))+'}\n[patch.crates-io]\nsdl2-sys={path='+json.dumps(str(work/'sdl2-sys-0.37.0'))+'}\n')
 output=subprocess.check_output(['cargo','run','--offline','--quiet','--release','--manifest-path',str(work/'Cargo.toml')],env=dict(os.environ,CARGO_TARGET_DIR='/tmp/gamepkg-bonus-sdl-reference-target'),text=True)
 cases=[]
 for line in output.splitlines():
  row=line.split()
  if row[0]=='CASE':
   which,position,x,y=map(int,row[1:]);cases.append(dict(kind=which,position=position,x=x,y=y,commands=[]))
  else:cases[-1]['commands'].append(line)
 if args.commands_only:
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),cases=[case for case in cases if case['position']==0]),indent=2));raise SystemExit(0)
 raster=Path(__file__).with_name('sdl_bonus_layers.c');flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split();subprocess.run(['cc',str(raster),'-o',str(work/'raster'),*flags],check=True)
 layers={};records=[];originals={}
 for case in cases:
  commands=('\n'.join(case['commands'])+'\n').encode();paths=[];names=[]
  assert len(case['commands'])==(2537 if case['kind']==1 else 2297)
  for index in range(4):
   rgba=bytearray(subprocess.check_output([str(work/'raster'),str(index),str(case['x']),str(case['y']),'00000000'],input=commands,env=render_env))
   if index==2:
    for at in range(0,len(rgba),4):
     opacity=rgba[at+3]
     for channel in range(3):rgba[at+channel]=min(255,(rgba[at+channel]*255+opacity//2)//opacity) if opacity else 0
   kind_name=['long','ball','ghost','rocket'][case['kind']];name=['bonus-body-'+kind_name+'.png','bonus-highlight.png','bonus-rings.png','bonus-symbol-'+kind_name+'.png'][index]
   value=dict(width=41,height=41,rgba_sha256=hashlib.sha256(rgba).hexdigest(),rgba_hex=rgba.hex())
   if name in layers:assert layers[name]==value,(name,case['position'])
   else:layers[name]=value
   path=work/name;path.write_bytes(rgba);paths.append(str(path));names.append(name)
  composite=[]
  for background in ['000000ff','ffffffff','0b1020ff','505064ff','00000000','12345678']:
   original=subprocess.check_output([str(work/'raster'),'5',str(case['x']),str(case['y']),background],input=commands,env=render_env)
   grouped=subprocess.check_output([str(work/'raster'),'4',str(case['x']),str(case['y']),background,*paths],input=commands,env=render_env)
   delta=max(abs(a-b) for a,b in zip(original,grouped));worst=max(range(len(original)),key=lambda i:abs(original[i]-grouped[i]));assert delta<=3,(case['kind'],case['position'],background,delta,(worst//4%41,worst//4//41),list(original[worst//4*4:worst//4*4+4]),list(grouped[worst//4*4:worst//4*4+4]),[list(Path(path).read_bytes()[worst//4*4:worst//4*4+4]) for path in paths])
   software_env=dict(render_env,GAMEPKG_BONUS_SOFTWARE='1')
   software_original=subprocess.check_output([str(work/'raster'),'5',str(case['x']),str(case['y']),background],input=commands,env=software_env)
   software_grouped=subprocess.check_output([str(work/'raster'),'4',str(case['x']),str(case['y']),background,*paths],input=commands,env=software_env)
   software_delta=max(abs(a-b) for a,b in zip(software_original,software_grouped))
   if background=='0b1020ff':
    if str(case['kind']) in originals:assert originals[str(case['kind'])]==original.hex()
    else:originals[str(case['kind'])]=original.hex()
   composite.append(dict(background=background,max_channel_delta=delta,software_max_channel_delta=software_delta,original_sha256=hashlib.sha256(original).hexdigest(),grouped_sha256=hashlib.sha256(grouped).hexdigest()))
  records.append(dict(kind=case['kind'],position=case['position'],x=case['x'],y=case['y'],layers=names,primitive_count=len(case['commands']),commands_sha256=hashlib.sha256(commands).hexdigest(),composites=composite))
 if args.write_assets:
  from PIL import Image
  destination=Path(args.write_assets);assert destination.is_dir()
  for name,value in layers.items():
   image=Image.frombytes('RGBA',(41,41),bytes.fromhex(value['rgba_hex']));a=image.getchannel('A').getbbox();b=image.convert('RGB').getbbox();bounds=(min(a[0],b[0]),min(a[1],b[1]),max(a[2],b[2]),max(a[3],b[3]));image.crop(bounds).save(destination/name)
 print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',rendering_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),entities_sha256=hashlib.sha256(entities.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),sdl2_version='0.37.0',canonical_renderer='opengl',original_dark_rgba=originals,layers=layers,cases=records),indent=2))
