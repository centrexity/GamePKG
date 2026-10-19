"""Execute the unchanged original cannon branch with real SDL/TTF."""
from pathlib import Path
import argparse,hashlib,json,os,subprocess,tempfile,tarfile,zlib,sys
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from gui_test_support import start_display
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--sdl-crate',required=True);parser.add_argument('--sdl-sys-crate',required=True);args=parser.parse_args()
source=args.original/'src/rendering.rs';assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
font=args.original/'assets/font/static/Roboto-Medium.ttf';assert hashlib.sha256(font.read_bytes()).hexdigest()=='616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'
text=source.read_text();a=text.index('    // Draw cannon on paddle');b=text.index('    // Draw balls (shiny',a);fragment=text[a:b]
with tempfile.TemporaryDirectory(prefix='gamepkg-cannon-reference-') as temp:
 root=Path(temp);(root/'src').mkdir()
 for archive,digest in [(args.sdl_crate,'3b498da7d14d1ad6c839729bd4ad6fc11d90a57583605f3b4df2cd709a9cd380'),(args.sdl_sys_crate,'951deab27af08ed9c6068b7b0d05a93c91f0a8eb16b6b816a5e73452a43521d3')]:
  assert hashlib.sha256(Path(archive).read_bytes()).hexdigest()==digest
  with tarfile.open(archive) as bundle:bundle.extractall(root,filter='data')
 (root/'Cargo.toml').write_text('[package]\nname="arkanoo-cannon-reference"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nsdl2={path='+json.dumps(str(root/'sdl2-0.37.0'))+',features=["ttf"]}\n[patch.crates-io]\nsdl2-sys={path='+json.dumps(str(root/'sdl2-sys-0.37.0'))+'}\n')
 rust=r'''use sdl2::{rect::Rect,pixels::{Color as SdlColor,PixelFormatEnum},video::Window,render::Canvas,ttf::Font};
struct Paddle{x:i32,y:i32,width:i32,rocket_ammo:u32}struct Game{paddle:Paddle,frame_count:u64}
fn draw(canvas:&mut Canvas<Window>,game:&Game,font:&Font){
'''+fragment+r'''}
fn main(){let args:Vec<String>=std::env::args().collect();let context=sdl2::init().unwrap();let video=context.video().unwrap();let ttf=sdl2::ttf::init().unwrap();let font=ttf.load_font(&args[1],26).unwrap();sdl2::hint::set("SDL_RENDER_SCALE_QUALITY","linear");let scale:f32=if args.len()>6 {args[6].parse().unwrap()} else {1.0};let window=video.window("original-cannon",(1280.0*scale) as u32,(720.0*scale) as u32).hidden().build().unwrap();let mut canvas=window.into_canvas().build().unwrap();assert_eq!(canvas.info().name,"opengl");canvas.set_scale(scale,scale).unwrap();canvas.set_draw_color(SdlColor::RGB(120,180,240));canvas.clear();canvas.set_draw_color(SdlColor::RGB(230,50,90));canvas.fill_rect(Rect::new(0,620,1280,50)).unwrap();draw(&mut canvas,&Game{paddle:Paddle{x:args[2].parse().unwrap(),y:670,width:args[3].parse().unwrap(),rocket_ammo:args[5].parse().unwrap()},frame_count:args[4].parse().unwrap()},&font);let pixels=canvas.read_pixels(None,PixelFormatEnum::RGBA32).unwrap();std::io::Write::write_all(&mut std::io::stdout(),&pixels).unwrap();}
'''
 (root/'src/main.rs').write_text(rust);target=Path('/tmp/gamepkg-cannon-reference-target');subprocess.run(['cargo','build','--offline','--quiet','--release','--manifest-path',root/'Cargo.toml'],env=dict(os.environ,CARGO_TARGET_DIR=str(target)),check=True)
 env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);display=start_display(env,width=1280,height=720)
 try:
  cases=[]
  for x,width in [(570,140),(430,280),(0,141),(1139,141),(-10,140)]:
   for frame in [0,29,30,59,60,18446744073709551615]:
    for ammo in [0,4294967295]:
     pixels=subprocess.check_output([target/'release/arkanoo-cannon-reference',font,str(x),str(width),str(frame),str(ammo)],env=env);assert len(pixels)==1280*720*4
     small=subprocess.check_output([target/'release/arkanoo-cannon-reference',font,str(x),str(width),str(frame),str(ammo),'0.5'],env=env);assert len(small)==640*360*4
     cases.append(dict(small_rgba_sha256=hashlib.sha256(small).hexdigest(),small_rgba_zlib_hex=zlib.compress(small,9).hex(),x=x,width=width,frame=frame,ammo=ammo,rgba_sha256=hashlib.sha256(pixels).hexdigest(),rgba_zlib_hex=zlib.compress(pixels,9).hex()))
  print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),size=[1280,720],scope='unchanged original cannon and blink caption over a contrast backdrop',cases=cases),indent=2))
 finally:display.terminate();display.wait(timeout=10)
