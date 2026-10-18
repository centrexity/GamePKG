"""Signed Arkanoo port: isolated builds, native keyboard play and browser export."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os, shutil, subprocess, tempfile, threading, time, tomllib, json
from PIL import Image
from gui_test_support import start_display,choose_native_file
import argparse, math, zlib
from contextlib import nullcontext
from collections import Counter
parser=argparse.ArgumentParser()
parser.add_argument('--keep-fixtures',action='store_true',help='Retain the isolated test directory for diagnosing failed signed checks')
parser.add_argument('--runtime-only',action='store_true',help='Run both signed execution profiles without starting GUI workflows')
parser.add_argument('--skip-browser',action='store_true',help='Run native checks on hosts without Chromium/Selenium')
args=parser.parse_args()
if not args.skip_browser and not args.runtime_only:
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.common.by import By
    from selenium.webdriver.common.action_chains import ActionChains
    from selenium.webdriver.common.keys import Keys
    from selenium.webdriver.support.ui import WebDriverWait
PROJECT=Path(__file__).resolve().parents[1];CLI=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'
PROOF=PROJECT/'target/arkanoo-proof';PROOF.mkdir(parents=True,exist_ok=True)
# Compile outside the timed gameplay run. Timed native IPC/rendering checks
# use the optimized production host; debug-host overhead is not game execution.
# Running the returned executable also excludes Cargo compile/lock waits.
compiled=subprocess.run(['cargo','test','--release','--test','game_arkanoo_runtime','--no-run','--message-format=json'],capture_output=True,text=True,timeout=300)
assert compiled.returncode==0,compiled.stderr+'\n'+compiled.stdout
executables=[value['executable'] for line in compiled.stdout.splitlines() if (value:=json.loads(line)).get('reason')=='compiler-artifact' and value.get('target',{}).get('name')=='game_arkanoo_runtime' and value.get('executable')]
assert len(executables)==1,executables
RUNTIME_TEST=Path(executables[0])
BRICK_REFERENCE=[bytes.fromhex(image['rgba_hex']) for image in json.loads((PROJECT/'tests/fixtures/arkanoo-blocks.json').read_text())['images']]
BRICK_SAMPLES=[(x,y) for y in [103,105,107] for x in [25,35,45]]
MENU_REFERENCE=zlib.decompress(bytes.fromhex(json.loads((PROJECT/'tests/fixtures/arkanoo-menu-controls.json').read_text())['base_rgba_zlib_hex']))
def original_menu_backdrop_matches(pixels):
    # Independent canonical SDL pixels, filtered down by two. Sample outside
    # controls; retain the same two-level renderer rounding bound as menu GUI QA.
    for x,y in [(70,270),(100,180),(490,120)]:
        sx=x*2;sy=(y-60)*2
        for channel in range(3):
            expected=sum(MENU_REFERENCE[((sy+dy)*1280+sx+dx)*4+channel] for dy in [0,1] for dx in [0,1])/4
            if abs(pixels[(y*640+x)*4+channel]-expected)>2:return False
    return True

def fire_core_visible(samples):
    # Reflections mix only white into the magma color. The orange/blue core
    # changes its hue, so it must create pixels off that white-reflection axis.
    candidates=Counter(c for c in samples if c[0]>=200 and 75<=c[1]<=125 and c[2]<=2)
    if not candidates:return False
    base=candidates.most_common(1)[0][0]
    return sum(c[2]>5 and max(abs(c[i]-(base[i]*(1-c[2]/255)+c[2])) for i in [0,1])>9 for c in samples)>=3
def original_brick_sample(color,x,y):
    # Opaque source pixels, bilinear filtering at the actual captured pixel center.
    left=math.floor(x);top=math.floor(y);fx=x-left;fy=y-top;pixels=BRICK_REFERENCE[color]
    return [sum(pixels[((top+dy)*60+left+dx)*4+c]*wx*wy for dx,wx in [(0,1-fx),(1,fx)] for dy,wy in [(0,1-fy),(1,fy)]) for c in range(3)]
def brick_rgb_matches(actual,expected):
    # Allow three 8-bit levels for renderer/capture interpolation rounding.
    return all(abs(actual[c]-expected[c])<=3 for c in range(3))
def native_brick_matches(image,color):
    scale=min(image.width/640,image.height/480);ox=(image.width-640*scale)/2;oy=(image.height-480*scale)/2
    for wx,wy in BRICK_SAMPLES:
        px=round(ox+wx*scale);py=round(oy+wy*scale)
        sx=((px+.5-ox)/scale-20)*2-.5;sy=((py+.5-oy)/scale-100)*2-.5
        if not brick_rgb_matches(image.getpixel((px,py)),original_brick_sample(color,sx,sy)):return False
    return True
def browser_brick_matches(pixels,color):
    return all(brick_rgb_matches(pixels[(y*640+x)*4:(y*640+x)*4+3],original_brick_sample(color,(x+.5-20)*2-.5,(y+.5-100)*2-.5)) for x,y in BRICK_SAMPLES)
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
with (nullcontext(tempfile.mkdtemp(prefix='gamepkg-arkanoo-',dir=PROJECT/'target')) if args.keep_fixtures else tempfile.TemporaryDirectory(prefix='gamepkg-arkanoo-',dir=PROJECT/'target')) as temp:
    if args.keep_fixtures:print('Retained isolated fixture directory:',temp,flush=True)
    root=Path(temp);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);env['DBUS_SESSION_BUS_ADDRESS']='unix:path=/tmp/gamepkg-no-session-bus';env['OMP_THREAD_LIMIT']='1'
    key=root/'key';key.write_text('61'*32)
    def run(*args):
        if args and args[0] in ('build','integrity-fix') and '--target' not in args: args=(*args,'--target','wasm')
        r=subprocess.run([str(CLI),*map(str,args)],env=env,text=True,capture_output=True,timeout=120)
        assert r.returncode==0,r.stderr
        return r.stdout
    run('pack',PROJECT/'ports/arkanoo','-o',root/'arkanoo.gamepkg','--key',key)
    run('install',root/'arkanoo.gamepkg','--trust-creator');run('build','com.gamepkg.arkanoo')
    result=run('run','com.gamepkg.arkanoo','--frames',180)
    assert 'Ran 180 frames' in result and 'draw commands: 1024' not in result,result
    run('integrity-check','com.gamepkg.arkanoo')
    nosave=root/'nosave';shutil.copytree(PROJECT/'ports/arkanoo',nosave)
    file=nosave/'manifest.toml';file.write_text(file.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-nosave').replace('save = true','save = false'))
    run('pack',nosave,'-o',root/'nosave.gamepkg','--key',key);run('install',root/'nosave.gamepkg','--trust-creator');run('build','com.gamepkg.arkanoo-nosave')
    custom=root/'custom';shutil.copytree(PROJECT/'ports/arkanoo',custom)
    file=custom/'manifest.toml';file.write_text(file.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-custom'))
    boards=json.loads((PROJECT/'tests/fixtures/arkanoo-custom-levels.json').read_text())['patterns']
    assert boards==[[i if i<9 else 255 for i in range(200)],[i//20%6 if i//20==i%20 else 255 for i in range(200)],[255]*200]
    (custom/'source/src/lib.rs').write_text('mod procedural;mod background;mod audio;mod editor;mod levels;mod menu;mod pattern_rng;mod physics;mod render;mod speed_font;mod editor_font;mod dynamic_font;mod dynamic_font_data;pub fn game_tick(input:u32,_:u64)->u64 {let mut game=physics::Game::new();for which in 0..3 {let mut board=[255u8;200];if which==0 {for col in 0..9 {board[col]=col as u8;}}if which==1 {for row in 0..10 {board[row*20+row]=(row%6) as u8;}}game.editor.patterns.push(editor::Pattern{name:String::new(),cells:board});}game.level=input;game.populate();game.mode=1;render::draw(&game);let mut result=14695981039346656037u64;for index in 0..200 {let cell=if game.blocks[index]==0 {255} else if game.block_type[index]>0 {game.block_type[index]+5} else {game.block_color[index]};result=(result^u64::from(cell)).wrapping_mul(1099511628211);}result}')
    run('pack',custom,'-o',root/'custom.gamepkg','--key',key);run('install',root/'custom.gamepkg','--trust-creator');run('build','com.gamepkg.arkanoo-custom')
    # Trigger the production portal at its original speed threshold without
    # depending on a long random play session. Only fixture setup is replaced.
    modes=root/'modes';shutil.copytree(PROJECT/'ports/arkanoo',modes)
    mode_manifest=modes/'manifest.toml';mode_manifest.write_text(mode_manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-modes'))
    (modes/'source/src/lib.rs').write_text('''mod procedural;mod background;mod audio;mod editor;mod levels;mod menu;mod pattern_rng;mod physics;mod render;mod speed_font;mod editor_font;mod dynamic_font;mod dynamic_font_data;use std::sync::Mutex;
static GAME:Mutex<Option<physics::Game>>=Mutex::new(None);
pub fn game_tick(input:u32,state:u64)->u64 {
 if input&268435456!=0 && input&2147483648==0 {let mut game=physics::Game::new();game.mode=if input&1!=0 {2} else {1};game.frame=u64::from(u32::MAX);game.update(0);return game.frame;};
 if input&67108864!=0 {
  let mut game=physics::Game::new();game.mode=1;
  for index in 0..200 {game.destroy(index);}game.update(0);
  if input&1!=0 {for (index,p) in game.particles.iter_mut().enumerate() {p.x=if index%2==0 {0.0} else {1279.0};p.y=if index%4<2 {0.0} else {719.0};}}
  render::draw(&game);return game.particles.len() as u64;
 }
 if input&33554432!=0 {
  let values=if input&3==0 {[100.75,200.25,2.0,-4.0,0.0,7.5,20.0,3.0]}
   else if input&3==1 {[-0.75,0.75,-2.5,3.0,45.0,-10.0,39.0,7.0]}
   else if input&3==2 {[1279.5,719.75,0.0,0.0,90.0,0.0,30.0,6.0]}
   else {[640.0,360.0,1.5,-2.25,359.0,9.5,21.0,5.0]};
  let mut p=physics::Particle{x:values[0],y:values[1],vx:values[2],vy:values[3],rotation:values[4],rotation_speed:values[5],life:values[6] as u32,max_life:values[6] as u32,size:values[7] as i32,color:0x1964c8ff};
  for _ in 0..((input>>2)&63) {p.advance();}
  gamepkg::draw_clear(0x000000ff);render::draw_particles(&[p]);return 1;
 }
 if input&16777216!=0 {let frame=if input&1!=0 {u64::from(u32::MAX)} else {u64::MAX};let samples=audio::effect_samples(1,frame,128,false);gamepkg::audio_submit(&samples);return 1;}
 if let Ok(mut slot)=GAME.lock(){
  if slot.is_none(){let mut game=physics::Game::new();game.mode=1;game.blocks=[0;200];game.blocks[0]=1;game.balls[0].attached=false;game.balls[0].vx=30.0;game.balls[0].vy=0.0;*slot=Some(game);}
  if let Some(game)=slot.as_mut(){
   if input&4096!=0 {game.portal=false;game.gravity=false;game.max_speed=0.0;game.mode=1;game.particles.clear();game.balls.clear();
    game.balls.push(physics::Ball{x:100.0,y:250.0,vx:1.0,vy:2.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});
    game.balls.push(physics::Ball{x:104.0,y:250.0,vx:-1.0,vy:-2.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});
   }
   if input&1024!=0 {game.portal=false;game.gravity=false;game.max_speed=0.0;game.mode=1;game.blocks=[0;200];game.blocks[0]=1;game.balls.clear();game.balls.push(physics::Ball{x:34.0,y:114.0,vx:0.0,vy:-4.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});}
   if input&2048!=0 {game.portal=false;game.gravity=false;game.max_speed=0.0;game.mode=1;game.blocks=[0;200];game.blocks[0]=1;game.balls.clear();
    game.balls.push(physics::Ball{x:300.0,y:396.0,vx:1.0,vy:2.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});
    game.balls.push(physics::Ball{x:304.0,y:396.0,vx:-1.0,vy:-2.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});
   }
   if input&8192!=0 {*game=physics::Game::new();game.mode=1;}
   if input&16384!=0 {*game=physics::Game::new();game.mode=1;game.level=9;game.width=90.0;game.long=300;game.ghost=600;game.ammo=1;game.score=100;game.balls.clear();}
   if input&32768!=0 {*game=physics::Game::new();game.mode=1;game.level=999;game.blocks=[0;200];game.score=10000004;}
   if input&65536!=0 {*game=physics::Game::new();game.mode=1;game.blocks=[0;200];game.set_block(0,1);game.set_block(1,3);game.set_block(2,2);game.set_block(199,0);game.balls.clear();game.balls.push(physics::Ball{x:34.0,y:114.0,vx:0.0,vy:-4.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});}
   if input&262144!=0 {game.balls.clear();game.balls.push(physics::Ball{x:34.0,y:114.0,vx:0.0,vy:-4.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});}
   if input&524288!=0 {*game=physics::Game::new();game.mode=1;game.blocks=[0;200];
    game.set_block(0,0);game.block_x[0]=20.0;game.block_y[0]=100.0;
    game.set_block(1,3);game.block_x[1]=20.0;game.block_y[1]=160.0;
    game.set_block(2,1);game.block_x[2]=20.0;game.block_y[2]=160.5;
    game.set_block(3,1);game.block_x[3]=56.0;game.block_y[3]=148.0;
    game.set_block(4,0);game.block_x[4]=56.5;game.block_y[4]=148.0;
    game.set_block(5,2);game.block_x[5]=80.0;game.block_y[5]=100.0;
    game.set_block(6,0);game.block_x[6]=140.0;game.block_y[6]=100.0;
    game.rockets.push((25.0,112.0));
   }
   if input&1048576!=0 {*game=physics::Game::new();game.mode=1;for index in 0..200 {game.set_block(index,1);}}
   if input&64!=0 {game.blocks=[0;200];}game.update(input);render::draw(game);
   let brick=game.balls.len()==1 && game.balls[0].y==113.0 && game.balls[0].vy==4.0 && game.blocks[0]==0;
   let ordered=game.balls.len()==2 && (game.balls[0].vx+4.4).abs()<0.001 && (game.balls[1].vx-0.8).abs()<0.001 && game.score==10;
   let contact=game.balls.len()==2 && game.balls[0].vx == -2.5 && game.balls[1].vx == 2.5 && game.balls[0].vy == -4.0 && game.balls[1].vy == -4.0;
   let waiting=game.balls.len()==1 && game.balls[0].attached;
   let stationary=game.balls.len()==1 && !game.balls[0].attached && game.balls[0].y==392.0 && game.balls[0].vy==-2.0 && game.balls[0].launch_timer==0;
   let preserved=game.width==90.0 && game.long>0 && game.ghost>0 && game.ammo==1 && game.lost_life;
   let ice=game.blocks[0]==1 && game.block_type[0]==1 && game.block_health[0]==1;
   let blast=game.blocks[0]==0 && game.blocks[1]==0 && game.blocks[2]==1 && game.blocks[3]==0 && game.blocks[4]==1 && game.blocks[5]==0 && game.blocks[6]==1;
   let score=if input&4096!=0 {game.particles.len() as u32} else {game.score};
   return (u64::from(score)<<32)|u64::from(game.portal)|(u64::from(game.gravity)<<1)|(u64::from(game.mode==3)<<2)|(u64::from(contact)<<3)|(u64::from(brick)<<4)|(u64::from(ordered)<<5)|(u64::from(waiting)<<6)|(u64::from(stationary)<<7)|(u64::from(game.mode==5)<<10)|(u64::from(preserved)<<11)|(u64::from(ice)<<12)|(u64::from(blast)<<13)|(u64::from(game.lives)<<8)|(u64::from(game.level)<<16);
  }
 }state
}''')
    trail_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-trails.json').read_text())['cases']
    metal_probe='fn metal_probe(input:u32)->u64 {if input&8!=0 {let mut game=physics::Game::new();game.mode=1;for index in 0..200 {game.set_block(index,3);}render::draw(&game);return 200;}let positions=[[0.0f32,0.0f32],[200.75,80.25],[-0.75,-0.75],[-1.75,-1.75]];let at=positions[(input&3) as usize];gamepkg::draw_clear(0x000000ff);u64::from(render::draw_metal_brick(at[0]*0.5,60.0+at[1]*0.5))}\n'
    ice_probe='fn ice_probe(input:u32)->u64 {if input&512!=0 {let mut game=physics::Game::new();game.mode=6;for _ in 0..3 {game.update(0);}game.mode=1;game.update(0);game.mode=6;for _ in 0..2 {game.update(0);}game.mode=2;game.update(0);game.mode=6;game.editor.cells[0]=6;game.editor.health[0]=2;render::draw(&game);return (game.editor.frame_count<<32)|game.frame;}if input&128!=0 {let mut game=physics::Game::new();game.mode=1;game.frame=20;for index in 0..200 {game.set_block(index,1);game.block_health[index]=1;}render::draw(&game);return 200;}let case=input&127;let frames=[0u64,1,5,10,20,30,59,60,119,120,16777217,18446744073709551615];let positions=[[40.0f32,80.0f32],[100.0,80.0],[640.0,260.0],[-1.0,-1.0]];let at=positions[((case/2)%4) as usize];let health=if case%2==0 {2} else {1};gamepkg::draw_clear(0x000000ff);u64::from(render::draw_ice_brick(health,2,frames[(case/8) as usize],at[0]*0.5,60.0+at[1]*0.5))}\n'
    fire_probe='fn fire_probe(input:u32)->u64 {if input&128!=0 {let mut game=physics::Game::new();game.mode=1;game.frame=[0u64,40,80][(input&3) as usize];for index in 0..200 {let kind=if input&64!=0 {(index%3+1) as u8} else {2};game.set_block(index,kind);if kind==1 {game.block_health[index]=1;}}render::draw(&game);return 200;}let case=input&63;let frames=[0u64,1,5,10,20,30,40,60,79,80,119,120,126,16777217,9007199254740993,18446744073709551615];let positions=[[40.0f32,80.0f32],[100.0,80.0],[640.0,260.0],[-1.0,-1.0]];let at=positions[(case%4) as usize];gamepkg::draw_clear(0x000000ff);u64::from(render::draw_fire_brick(frames[(case/4) as usize],at[0]*0.5,60.0+at[1]*0.5))}\n'
    bonus_probe='fn bonus_probe(input:u32)->u64 {if input&128!=0 {let mut game=physics::Game::new();game.mode=1;game.blocks=[0;200];for index in 0..200 {game.rockets.push((20.0+(index%20) as f32*30.0,120.0+(index/20) as f32*22.0));}render::draw(&game);return 200;}if input&64!=0 {let positions=[[0.0f32,0.0f32],[320.75,240.25],[-0.75,-0.75],[-1.75,-1.75],[1279.5,719.75]];let at=positions[(input&7) as usize];gamepkg::draw_clear(0x000000ff);return u64::from(render::draw_rocket(at[0]*0.5,60.0+at[1]*0.5));}if input&32!=0 {let mut game=physics::Game::new();game.mode=1;game.blocks=[0;200];game.drops.clear();for index in 0..200 {game.drops.push(physics::Drop{x:20.0+(index%20) as f32*30.0,y:120.0+(index/20) as f32*22.0,kind:index%4});}render::draw(&game);return 200;}if input&16!=0 {gamepkg::draw_clear(0x000000ff);return u64::from(render::draw_bonus_item(4,0.0,60.0));}let positions=[[0.0f32,0.0f32],[320.75,240.25],[-0.75,-0.75],[-1.75,-1.75]];let at=positions[(input&3) as usize];gamepkg::draw_clear(0x000000ff);u64::from(render::draw_bonus_item((input&15)/4,at[0]*0.5,60.0+at[1]*0.5))}\n'
    portal_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-portal.json').read_text())['cases']
    portal_probe='fn portal_probe(input:u32)->u64 {let cases=['+','.join('['+str(case['frame'])+'u64,'+str(case['timer'])+'u64]' for case in portal_cases)+'];let at=cases[(input&31) as usize];if input&32!=0 {let mut game=physics::Game::new();game.mode=1;game.portal=true;game.frame=at[0];game.portal_timer=at[1] as u32;render::draw(&game);}else {gamepkg::draw_clear(0x0b1020ff);render::draw_portal(at[0],at[1]);}1}\n'
    background_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-background.json').read_text())['cases']
    background_probe='fn background_probe(input:u32)->u64 {let cases=['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in background_cases)+'];let at=cases[(input&31) as usize];if input&32!=0 {let mut game=physics::Game::new();game.mode=1;game.level=at[0] as u32;game.frame=at[1];for index in 0..200 {game.set_block(index,2);}for index in 0..3000 {game.particles.push(physics::Particle{x:if index%2==0 {0.0} else {1279.0},y:if index%4<2 {0.0} else {719.0},vx:0.0,vy:0.0,life:30,max_life:30,rotation:0.0,rotation_speed:0.0,size:7,color:0x1964c8ff});}render::draw(&game);3000}else {u64::from(render::draw_background(at[0] as u32,at[1]))}}\n'
    procedural_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-procedural.json').read_text())['cases']
    procedural_probe='fn procedural_probe(input:u32)->u64 {let cases=['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in procedural_cases)+'];let at=cases[(input&63) as usize];if input&64!=0 {let mut game=physics::Game::new();game.mode=1;game.level=at[0] as u32;game.frame=at[1];for index in 0..200 {game.set_block(index,2);}for index in 0..3000 {game.particles.push(physics::Particle{x:if index%2==0 {0.0} else {1279.0},y:if index%4<2 {0.0} else {719.0},vx:0.0,vy:0.0,life:30,max_life:30,rotation:0.0,rotation_speed:0.0,size:7,color:0x1964c8ff});}render::draw(&game);3000}else {u64::from(render::draw_background(at[0] as u32,at[1]))}}\n'
    fire_sea_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-fire-sea-background.json').read_text())['cases']
    old_cases='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in procedural_cases)+']'
    new_cases='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in fire_sea_cases)+']'
    fire_sea_probe=procedural_probe.replace('fn procedural_probe','fn fire_sea_probe').replace(old_cases,new_cases)
    space_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-space-background.json').read_text())['cases']
    space_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in space_cases)+']'
    space_probe=procedural_probe.replace('fn procedural_probe','fn space_probe').replace(old_cases,space_values)
    voyage_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-star-voyage.json').read_text())['cases']
    voyage_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in voyage_cases)+']'
    voyage_probe=procedural_probe.replace('fn procedural_probe','fn voyage_probe').replace(old_cases,voyage_values)
    line_cases=json.loads((PROJECT/'tests/fixtures/sdl-background-lines.json').read_text())['cases']
    line_values='['+','.join('['+','.join(str(value)+'i32' for value in case['segment'])+']' for case in line_cases)+']'
    line_probe='fn line_probe(input:u32)->u64 {let cases='+line_values+';let at=cases[(input&31) as usize];let mut dots=Vec::new();background::background_line_points(&mut dots,[at[0],at[1]],[at[2],at[3]],[64,64],0xc864ff96);gamepkg::draw_clear(0x0b1020ff);if let Some(image)=physics::effect_image(&dots){gamepkg::draw_image(image.left as f32*0.5,60.0+image.top as f32*0.5,image.width as f32*0.5,image.height as f32*0.5,image.width,image.height,&image.pixels);}dots.len() as u64}\n'
    kaleidoscope_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-kaleidoscope.json').read_text())['cases']
    kaleidoscope_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in kaleidoscope_cases)+']'
    kaleidoscope_probe=procedural_probe.replace('fn procedural_probe','fn kaleidoscope_probe').replace(old_cases,kaleidoscope_values)
    rain_warp_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-rain-warp.json').read_text())['cases']
    rain_warp_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in rain_warp_cases)+']'
    rain_warp_probe=procedural_probe.replace('fn procedural_probe','fn rain_warp_probe').replace(old_cases,rain_warp_values)
    cyber_aurora_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-cyber-aurora.json').read_text())['cases']
    cyber_aurora_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in cyber_aurora_cases)+']'
    cyber_aurora_probe=procedural_probe.replace('fn procedural_probe','fn cyber_aurora_probe').replace(old_cases,cyber_aurora_values)
    matrix_nebula_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-matrix-nebula.json').read_text())['cases']
    matrix_nebula_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in matrix_nebula_cases)+']'
    matrix_nebula_probe=procedural_probe.replace('fn procedural_probe','fn matrix_nebula_probe').replace(old_cases,matrix_nebula_values)
    hex_bubble_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-hex-bubbles.json').read_text())['cases']
    hex_bubble_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in hex_bubble_cases)+']'
    hex_bubble_probe=procedural_probe.replace('fn procedural_probe','fn hex_bubble_probe').replace(old_cases,hex_bubble_values)
    night_circuit_maze_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-night-circuit-maze.json').read_text())['cases']
    night_circuit_maze_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in night_circuit_maze_cases)+']'
    night_circuit_maze_probe=procedural_probe.replace('fn procedural_probe','fn night_circuit_maze_probe').replace(old_cases,night_circuit_maze_values)
    canonical_particle_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-canonical-particles.json').read_text())['cases']
    canonical_particle_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in canonical_particle_cases)+']'
    canonical_particle_probe=procedural_probe.replace('fn procedural_probe','fn canonical_particle_probe').replace(old_cases,canonical_particle_values)
    canonical_effect_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-canonical-effects.json').read_text())['cases']
    canonical_effect_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in canonical_effect_cases)+']'
    canonical_effect_probe=procedural_probe.replace('fn procedural_probe','fn canonical_effect_probe').replace(old_cases,canonical_effect_values)
    canonical_campaign_cases=json.loads((PROJECT/'tests/fixtures/arkanoo-canonical-campaign.json').read_text())['cases']
    canonical_campaign_values='['+','.join('['+str(case['level'])+'u64,'+str(case['frame'])+'u64]' for case in canonical_campaign_cases)+']'
    canonical_campaign_probe=procedural_probe.replace('fn procedural_probe','fn canonical_campaign_probe').replace(old_cases,canonical_campaign_values)
    probe=' if input&2684354560==2684354560 {let level=(input&7)+1;if input&512!=0 {let mut game=physics::Game::new();game.mode=6;game.editor.background=level;if input&1024!=0 {game.editor.cells=[8;200];game.editor.health=[3;200];}render::draw(&game);return u64::from(level);}if input&256!=0 {let mut game=physics::Game::new();game.mode=1;game.level=level;for index in 0..200 {game.set_block(index,2);}for index in 0..3000 {game.particles.push(physics::Particle{x:if index%2==0 {0.0} else {1279.0},y:if index%4<2 {0.0} else {719.0},vx:0.0,vy:0.0,life:30,max_life:30,rotation:0.0,rotation_speed:0.0,size:7,color:0x1964c8ff});}render::draw(&game);return 3000;}return u64::from(render::draw_background(level,0));}\n'
    probe+=' if input&2415919104==2415919104 {let positions=[[0.0f32,0.0f32],[320.75,240.25],[-0.75,-0.75],[-1.75,-1.75]];let at=positions[(input&3) as usize];let velocities=[[0.0f32,0.0f32],[0.5,0.0],[1.5,2.0],[5.0,0.0],[10.0,0.0],[0.0,15.0]];let velocity=velocities[((input&31)/4) as usize];let mut ball=physics::Ball::new(at[0]*0.5+3.0,at[1]*0.5+63.0);ball.vx=velocity[0];ball.vy=velocity[1];gamepkg::draw_clear(0x000000ff);return u64::from(render::draw_ball_speed(&ball));}\n'
    probe+=' if input&2281701376==2281701376 {return canonical_campaign_probe(input); }\n'
    probe+=' if input&2214592512==2214592512 {return canonical_effect_probe(input); }\n'
    probe+=' if input&2181038080==2181038080 {return canonical_particle_probe(input); }\n'
    probe+=' if input&2164260864==2164260864 {return night_circuit_maze_probe(input); }\n'
    probe+=' if input&2155872256==2155872256 {return hex_bubble_probe(input); }\n'
    probe+=' if input&2151677952==2151677952 {return matrix_nebula_probe(input); }\n'
    probe+=' if input&2149580800==2149580800 {return cyber_aurora_probe(input); }\n'
    probe+=' if input&2148532224==2148532224 {return rain_warp_probe(input); }\n'
    probe+=' if input&2148007936==2148007936 {return kaleidoscope_probe(input); }\n'
    probe+=' if input&2147745792==2147745792 {return line_probe(input); }\n'
    probe+=' if input&2147614720==2147614720 {return voyage_probe(input); }\n'
    probe+=' if input&2147549184==2147549184 {return space_probe(input); }\n'
    probe+=' if input&2147516416==2147516416 {return fire_sea_probe(input); }\n'
    probe+=' if input&2147500032==2147500032 {return procedural_probe(input); }\n'
    probe+=' if input&2147491840==2147491840 {return background_probe(input); }\n'
    probe+=' if input&2147487744==2147487744 {return portal_probe(input); }\n'
    probe+=' if input&2147485696==2147485696 {return bonus_probe(input); }\n'
    probe+=' if input&2147484672==2147484672 {return fire_probe(input); }\n'
    probe+=' if input&2147483904==2147483904 {return ice_probe(input); }\n'
    probe+=' if input&2147483776==2147483776 {return metal_probe(input);}\n'
    probe+=' if input&2147483664==2147483664 {let positions=[[0.0f32,0.0f32],[200.75,80.25],[-0.75,-0.75],[-1.75,-1.75]];let at=positions[((input>>5)&3) as usize];gamepkg::draw_clear(0x000000ff);return u64::from(render::draw_normal_brick((input&7) as u8,at[0]*0.5,60.0+at[1]*0.5));}\n'
    probe+=' if input&2147483656==2147483656 {let mut game=physics::Game::new();game.paddle=100.75;let variant=input&7;if variant==1 {game.width=90.0;}if variant==2 {game.ghost=600;}if variant>=3 {game.paddle_glow=1.0;}if variant==5 {game.paddle_glow=0.1;}if variant==6 {game.paddle_glow=0.2;}if variant==4 {game.last_paddle=game.paddle;game.update_paddle(0);}if variant==7 {game.paddle_glow=0.0;game.mode=1;game.last_paddle=game.paddle-2.0;game.balls.clear();game.balls.push(physics::Ball{x:game.paddle+35.0,y:394.0,vx:0.0,vy:2.0,spin:0.0,attached:false,launch_timer:0,trail:Vec::new()});game.update(0);}gamepkg::draw_clear(0x000000ff);return u64::from(render::draw_paddle(&game));}\n'
    probe+=' if input&2147483648!=0 {let positions=[[0.0f32,0.0f32],[320.75,240.25],[-0.75,-0.75],[-1.75,-1.75]];let at=positions[(input&3) as usize];let ball=physics::Ball::new(at[0]*0.5+3.0,at[1]*0.5+63.0);gamepkg::draw_clear(0x000000ff);return u64::from(render::draw_ball(&ball));}\n'
    visual_probe='fn visual_probe(input:u32)->u64 {'+probe+'0}\n'
    probe=' if input&2147483648!=0 {return visual_probe(input);}\n'
    probe+=' if input&134217728!=0 {\n'
    for index,case in enumerate(trail_cases):
        x,y,vx,vy=case['ball']
        probe+=f'if input&255=={index} {{let mut ball=physics::Ball::new({x}f32*0.5+3.0,{y}f32*0.5+63.0);ball.vx={vx}f32*0.5;ball.vy={vy}f32*0.5;'
        for x,y in case['trail']:probe+=f'ball.trail.push([{x}f32,{y}f32]);'
        probe+=f'gamepkg::draw_clear(0x000000ff);render::draw_ball_effects(&ball,{case["frame"]});return 1;}};\n'
    probe+='return 0;}\n'
    probe_file=modes/'source/src/lib.rs'
    text=probe_file.read_text();text=text.replace(' if input&67108864!=0 {',probe+' if input&67108864!=0 {');probe_file.write_text(text+metal_probe+ice_probe+fire_probe+bonus_probe+portal_probe+background_probe+procedural_probe+fire_sea_probe+space_probe+voyage_probe+line_probe+kaleidoscope_probe+rain_warp_probe+cyber_aurora_probe+matrix_nebula_probe+hex_bubble_probe+night_circuit_maze_probe+canonical_particle_probe+canonical_effect_probe+canonical_campaign_probe+visual_probe)
    penguin_probe='''
static HEART:Mutex<Option<physics::Game>>=Mutex::new(None);
fn penguin_probe(input:u32)->u64 {
 if let Ok(mut slot)=HEART.lock(){
  let reset=input&1!=0;
  if slot.is_none()||reset {let mut game=physics::Game::new();game.mode=1;game.lives=((input>>4)&3).max(1);game.score=100;game.balls.clear();game.update(0);*slot=Some(game);}
  if let Some(game)=slot.as_mut(){
   if input&2!=0 {game.mode=2;}if input&4!=0 {game.mode=1;}
   if !reset {if let Some(ball)=game.balls.first_mut(){ball.launch_timer=u32::MAX;}game.update(0);}
   render::draw(game);
   let mut value=u64::from(game.lives)|(u64::from(game.stolen_heart.is_some())<<8)|(u64::from(game.mode)<<12);
   if let Some(penguin)=&game.penguin {value|=(u64::from(penguin.state)<<16)|(1<<20)|(u64::from(penguin.frame_count)<<24);}else {value|=4<<16;}
   return value;
  }
 }0
}
'''
    api_probe=''' if input&536870912!=0 && input&2147483648==0 {
 let mut value=0u64;
 if gamepkg::draw_asset_image_linear("heart.png",0.0,60.0,10.0,10.0){value|=1;}
 if gamepkg::draw_asset_image("heart.png",20.0,60.0,10.0,10.0){value|=2;}
 if !gamepkg::draw_asset_image_linear("../heart.png",0.0,0.0,10.0,10.0){value|=4;}
 if !gamepkg::draw_asset_image_linear("/etc/passwd",0.0,0.0,10.0,10.0){value|=8;}
 if !gamepkg::draw_asset_image_linear("heart.png",f32::NAN,0.0,10.0,10.0){value|=16;}
 if !gamepkg::draw_asset_image_linear("heart.png",0.0,0.0,1281.0,10.0){value|=32;}
 let guard=[0u8];value+=u64::from(guard[(input&1) as usize]);return value;
 };
'''
    text=probe_file.read_text();text=text.replace('pub fn game_tick(input:u32,state:u64)->u64 {','pub fn game_tick(input:u32,state:u64)->u64 {\n if input&3221225472==3221225472 {let case=input&255;if case==68 {let states=[[0u32,0u32,0u32,64u32,0u32,0u32],[0u32,0u32,1u32,64u32,0u32,0u32],[0u32,0u32,2u32,64u32,0u32,0u32],[0u32,0u32,3u32,64u32,0u32,0u32],[0u32,0u32,4u32,64u32,0u32,0u32],[0u32,0u32,5u32,64u32,0u32,0u32],[0u32,0u32,6u32,64u32,0u32,0u32],[0u32,1u32,0u32,64u32,0u32,0u32],[0u32,1u32,1u32,64u32,0u32,0u32],[0u32,1u32,2u32,64u32,0u32,0u32],[0u32,1u32,3u32,64u32,0u32,0u32],[0u32,1u32,4u32,64u32,0u32,0u32],[0u32,1u32,5u32,64u32,0u32,0u32],[0u32,1u32,6u32,64u32,0u32,0u32],[1u32,0u32,0u32,0u32,0u32,0u32],[1u32,0u32,7u32,0u32,0u32,1u32],[1u32,0u32,10u32,0u32,1u32,0u32],[1u32,0u32,8u32,0u32,1u32,1u32],[1u32,0u32,0u32,1u32,0u32,0u32],[1u32,0u32,7u32,1u32,0u32,1u32],[1u32,0u32,10u32,1u32,1u32,0u32],[1u32,0u32,8u32,1u32,1u32,1u32],[1u32,0u32,0u32,63u32,0u32,0u32],[1u32,0u32,7u32,63u32,0u32,1u32],[1u32,0u32,10u32,63u32,1u32,0u32],[1u32,0u32,8u32,63u32,1u32,1u32],[1u32,0u32,0u32,64u32,0u32,0u32],[1u32,0u32,7u32,64u32,0u32,1u32],[1u32,0u32,10u32,64u32,1u32,0u32],[1u32,0u32,8u32,64u32,1u32,1u32],[1u32,0u32,0u32,127u32,0u32,0u32],[1u32,0u32,7u32,127u32,0u32,1u32],[1u32,0u32,10u32,127u32,1u32,0u32],[1u32,0u32,8u32,127u32,1u32,1u32],[1u32,0u32,0u32,128u32,0u32,0u32],[1u32,0u32,7u32,128u32,0u32,1u32],[1u32,0u32,10u32,128u32,1u32,0u32],[1u32,0u32,8u32,128u32,1u32,1u32]];let index=((input>>8)&63) as usize;if index>=states.len() {return 0;}let values=states[index];let mut game=physics::Game::new();game.mode=0;game.menu.settings=values[0]!=0;game.started=values[1]!=0;game.menu.hover=values[2];game.menu.sfx_volume=values[3];game.menu.sfx_muted=values[4]!=0;game.menu.fullscreen=values[5]!=0;render::draw_menu(&game);return 68;};if case==66 || case==67 {let names=["café","j́","Ελληνικά","Привет","日本語 🎮","// “Δ","aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","ä́","last","x"];let mut game=physics::Game::new();game.mode=6;game.editor.browser=true;game.editor.selection=if case==67 {9} else {((input>>16)&15) as usize};let count=if case==67 {10} else {((input>>8)&15) as usize};for label in names.iter().take(count) {game.editor.patterns.push(editor::Pattern{name:String::from(*label),cells:[255;200]});}if case==67 {for index in 0..200 {game.editor.cells[index]=8;game.editor.health[index]=1;}render::draw(&game);} else {gamepkg::draw_clear(0x78b4f0ff);gamepkg::draw_rect(0.0,135.0,640.0,100.0,0xe6325aff);render::draw_pattern_browser(&game.editor);}return u64::from(case);};let mut game=physics::Game::new();game.mode=6;if case==65 {let name_choice=(input>>8)&3;let chosen_name=match name_choice {0=>"my_pattern",1=>"café",2=>"Ελληνικά Привет 日本語 🎮 // “Δ",_=>"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"};game.editor.name=String::from(chosen_name);game.editor.naming=name_choice==1;game.editor.message=String::from("Saved: café");game.editor.message_time=180;game.editor.patterns.push(editor::Pattern{name:String::from(chosen_name),cells:[255;200]});game.editor.browser=true;}if case==64 {game.editor.hovered_button=((input>>8)&255) as u8;game.editor.background=((input>>16)&7)+1;}game.editor.selected=if case<27 {(case/3) as u8} else {255};game.editor.hovered_color=if case<27 {match case%3 {1=>((case/3+1)%9) as u8,2=>(case/3) as u8,_=>255}} else {255};render::draw(&game);return u64::from(case);}; if input&1073741824!=0 {return penguin_probe(input);};\n'+api_probe)
    probe_file.write_text(text+penguin_probe)
    run('pack',modes,'-o',root/'modes.gamepkg','--key',key);run('install',root/'modes.gamepkg','--trust-creator');run('build','com.gamepkg.arkanoo-modes')
    layouts=root/'layouts';shutil.copytree(PROJECT/'ports/arkanoo',layouts)
    layout_manifest=layouts/'manifest.toml';layout_manifest.write_text(layout_manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-levels'))
    (layouts/'source/src/lib.rs').write_text('''mod procedural;mod background;mod audio;mod editor;mod levels;mod menu;mod pattern_rng;mod physics;mod render;mod speed_font;mod editor_font;mod dynamic_font;mod dynamic_font_data;use std::sync::Mutex;
static GAME:Mutex<Option<physics::Game>>=Mutex::new(None);
pub fn game_tick(input:u32,state:u64)->u64 {
 if let Ok(mut slot)=GAME.lock(){
  if slot.is_none(){*slot=Some(physics::Game::new());}
  if let Some(game)=slot.as_mut(){game.level=input.max(1);game.populate();game.mode=1;render::draw(game);return u64::from(game.level);}
 }state
}''')
    run('pack',layouts,'-o',root/'layouts.gamepkg','--key',key);run('install',root/'layouts.gamepkg','--trust-creator');run('build','com.gamepkg.arkanoo-levels')
    for suffix,profile,allowed in [('events','core-v1',True),('events-denied','core-v1',False),('events-component','component-v1',True),('events-component-denied','component-v1',False)]:
        fixture=root/suffix;shutil.copytree(PROJECT/'ports/arkanoo',fixture)
        manifest=fixture/'manifest.toml';text=manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-'+suffix).replace('runtime_profile = "core-v1"','runtime_profile = "'+profile+'"')
        if 'runtime_profile =' not in text:text=text.replace('entry_world = "game"','runtime_profile = "'+profile+'"\nentry_world = "game"')
        assert tomllib.loads(text)['runtime_profile']==profile
        if not allowed:text=text.split('[[permissions.files]]')[0].replace('input = true','input = false')
        manifest.write_text(text)
        (fixture/'source/src/lib.rs').write_text('pub fn game_tick(_:u32,_:u64)->u64 {if let Some(events)=gamepkg::input_events(){let mut result=0;for event in events {if event.kind==1 {gamepkg::draw_rect(event.x,event.y,1.0,1.0,0xffffffff);}result=(u64::from(event.kind)<<32)|u64::from(event.code);}result}else{u64::MAX}}')
        run('pack',fixture,'-o',root/(suffix+'.gamepkg'),'--key',key);run('install',root/(suffix+'.gamepkg'),'--trust-creator');run('build','com.gamepkg.arkanoo-'+suffix)
    for suffix,profile,allowed in [('files-component','component-v1',True),('nosave-component','component-v1',True),('nofiles','core-v1',False)]:
        fixture=root/suffix;shutil.copytree(PROJECT/'ports/arkanoo',fixture)
        manifest=fixture/'manifest.toml';text=manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-'+suffix)
        text='runtime_profile = "'+profile+'"\n'+text
        if not allowed:text=text.split('[[permissions.files]]')[0]
        if suffix=='nosave-component':text=text.replace('save = true','save = false')
        manifest.write_text(text);run('pack',fixture,'-o',root/(suffix+'.gamepkg'),'--key',key);run('install',root/(suffix+'.gamepkg'),'--trust-creator');run('build','com.gamepkg.arkanoo-'+suffix)
    probe_env=dict(env,GAMEPKG_ARKANOO_TEST_ROOT=str(root/'library'))
    subprocess.run([str(RUNTIME_TEST),'--ignored'],env=probe_env,check=True,timeout=120)
    run('build','com.gamepkg.arkanoo','--target','native')
    run('build','com.gamepkg.arkanoo-nosave','--target','native')
    run('build','com.gamepkg.arkanoo-nofiles','--target','native')
    run('build','com.gamepkg.arkanoo-custom','--target','native')
    run('build','com.gamepkg.arkanoo-modes','--target','native')
    run('build','com.gamepkg.arkanoo-levels','--target','native')
    run('build','com.gamepkg.arkanoo-events','--target','native')
    run('build','com.gamepkg.arkanoo-events-denied','--target','native')
    run('integrity-check','com.gamepkg.arkanoo')
    subprocess.run([str(RUNTIME_TEST),'--ignored'],env=probe_env,check=True,timeout=120)
    if args.runtime_only:
        print('Signed Arkanoo checks passed in Wasm and native execution; GUI workflows were not requested.')
        raise SystemExit(0)
    # Real native window: locate the original cyan core, hold movement, start, pause.
    display=start_display(env,width=1400,height=900);app=None;wm=None
    from Xlib import display as xdisplay
    from Xlib.ext import xfixes
    def cursor_visible():
        connection=xdisplay.Display(env['DISPLAY'])
        try:
            connection.xfixes_query_version()
            return any(connection.xfixes_get_cursor_image(connection.screen().root).cursor_image)
        finally:connection.close()
    def expect_cursor(visible):
        deadline=time.monotonic()+5
        while cursor_visible()!=visible:
            assert time.monotonic()<deadline,('Cursor visibility',visible);time.sleep(.05)
    def capture(window,name):
        path=PROOF/name
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10)
        return Image.open(path).convert('RGB')
    def native_click(window,x,y,button=1):
        screenshot=capture(window,'native-menu.png');scale=min(screenshot.width/640,screenshot.height/480)
        px=round((screenshot.width-640*scale)/2+x*scale);py=round((screenshot.height-480*scale)/2+y*scale)
        subprocess.run(['xdotool','mousemove','--window',window,str(px),str(py),'click',str(button)],env=env,check=True)
        time.sleep(.15)
    def native_slider(window,x):
        screenshot=capture(window,'native-sfx-settings.png');scale=min(screenshot.width/640,screenshot.height/480)
        ox=(screenshot.width-640*scale)/2;oy=(screenshot.height-480*scale)/2
        subprocess.run(['xdotool','mousemove','--window',window,str(math.ceil(ox+(319 if x==320 else 320)*scale)),str(round(oy+245*scale)),'mousedown','1'],env=env,check=True)
        subprocess.run(['xdotool','mousemove','--window',window,str(math.ceil(ox+x*scale)),str(round(oy+245*scale))],env=env,check=True);time.sleep(.15)
        subprocess.run(['xdotool','mouseup','1'],env=env,check=True)
    def native_sfx(volume,muted):
        deadline=time.monotonic()+5
        while True:
            values=[int.from_bytes(path.read_bytes()[12:16],'little') for path in (root/'library/saves').glob('*.blob')]
            if any(((value>>1)&255)==volume and bool(value&512)==muted for value in values):return
            assert time.monotonic()<deadline,('Native audio settings did not persist',values,volume,muted);time.sleep(.05)
    def paddle(image):
        scale=min(image.width/640,image.height/480);oy=(image.height-480*scale)/2
        # The original alpha-blended cyan core replaces the flat opaque body.
        # Restrict the scan to its world-space strip; tolerate host color-space
        # blending and filtered scaling while retaining the movement assertion.
        points=[]
        for y in range(math.ceil(oy+399*scale),math.floor(oy+401*scale)):
            for x in range(image.width):
                r,g,b=image.getpixel((x,y))
                if r<50 and g>140 and b>190 and g-r>80 and b-g>20:points.append((x,y))
        assert len(points)>40*scale*scale,len(points)
        return sum(x for x,y in points)/len(points)
    try:
        wm=subprocess.Popen(['metacity','--sm-disable','--replace','--no-composite'],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        deadline=time.monotonic()+5
        while '_NET_SUPPORTING_WM_CHECK' not in subprocess.run(['xprop','-root','_NET_SUPPORTING_WM_CHECK'],env=env,capture_output=True,text=True).stdout or 'not found' in subprocess.run(['xprop','-root','_NET_SUPPORTING_WM_CHECK'],env=env,capture_output=True,text=True).stdout:
            assert time.monotonic()<deadline,'Private test window manager did not start';time.sleep(.05)
        app=subprocess.Popen([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg-gui'),'--play','com.gamepkg.arkanoo'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        deadline=time.monotonic()+20
        while time.monotonic()<deadline:
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            found=subprocess.run(['xdotool','search','--onlyvisible','--name',r'^(GamePKG|com\..*)$'],env=env,capture_output=True,text=True,timeout=2)
            if found.returncode==0:break
            time.sleep(.1)
        assert found.returncode==0
        window=found.stdout.splitlines()[0];subprocess.run(['xdotool','windowfocus',window],env=env,check=True);time.sleep(.5)
        ready=time.monotonic()+20
        while True:
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            try:
                title=capture(window,'native-title.png')
                assert sum(count for count,color in title.getcolors(title.width*title.height) if color==(104,229,255))>100
                break
            except AssertionError:
                assert time.monotonic()<ready,'The actual game frame did not become ready'
                time.sleep(.1)
        subprocess.run(['xdotool','key','Return'],env=env,check=True);time.sleep(.15)
        native_click(window,320,285) # Original Settings button.
        native_slider(window,295);native_sfx(32,False)
        native_click(window,320,225);native_sfx(32,True)
        native_click(window,320,225);native_slider(window,320);native_sfx(64,False)
        native_click(window,320,270) # Its fullscreen action affects the real window.
        deadline=time.monotonic()+5
        while '_NET_WM_STATE_FULLSCREEN' not in subprocess.run(['xprop','-id',window,'_NET_WM_STATE'],env=env,capture_output=True,text=True).stdout:
            assert time.monotonic()<deadline,'Settings did not enter actual fullscreen';time.sleep(.05)
        native_click(window,320,270)
        deadline=time.monotonic()+5
        while '_NET_WM_STATE_FULLSCREEN' in subprocess.run(['xprop','-id',window,'_NET_WM_STATE'],env=env,capture_output=True,text=True).stdout:
            assert time.monotonic()<deadline,'Settings did not leave actual fullscreen';time.sleep(.05)
        native_click(window,320,450)
        native_click(window,320,185) # Original New Game button.

        ready=time.monotonic()+10
        while True:
            before=capture(window,'native-start.png')
            try:
                paddle(before)
                break
            except AssertionError:
                assert time.monotonic()<ready,'The original-sized paddle was not visible after start'
                time.sleep(.05)
        subprocess.run(['xdotool','keydown','--window',window,'Right'],env=env,check=True)
        try:
            moved=time.monotonic()+3
            while True:
                after=capture(window,'native-right.png')
                if paddle(after)>paddle(before)+25:break
                assert time.monotonic()<moved,'Held Right did not move the paddle'
                time.sleep(.05)
        finally:subprocess.run(['xdotool','keyup','--window',window,'Right'],env=env,check=True)
        target_x=after.width//4;target_y=after.height*3//5
        subprocess.run(['xdotool','mousemove','--window',window,str(target_x),str(target_y)],env=env,check=True)
        moved=time.monotonic()+3
        while True:
            mouse=capture(window,'native-mouse.png')
            if abs(paddle(mouse)-target_x)<10:break
            assert time.monotonic()<moved,'Mouse motion did not center the original paddle'
            time.sleep(.05)
        subprocess.run(['xdotool','key','--window',window,'Escape'],env=env,check=True);time.sleep(.2)
        capture(window,'native-paused.png');expect_cursor(True)
        subprocess.run(['xdotool','key','--window',window,'e'],env=env,check=True);time.sleep(.2)
        editor_screen=capture(window,'native-editor-empty.png');scale=min(editor_screen.width/640,editor_screen.height/480)
        ox=(editor_screen.width-640*scale)/2;oy=(editor_screen.height-480*scale)/2
        px=round(ox+35*scale);py=round(oy+105*scale)
        subprocess.run(['xdotool','key','--window',window,'1'],env=env,check=True)
        subprocess.run(['xdotool','mousemove','--window',window,str(px),str(py),'click','1'],env=env,check=True)
        deadline=time.monotonic()+3
        while True:
            editor_screen=capture(window,'native-editor-painted.png')
            if native_brick_matches(editor_screen,1):break
            assert time.monotonic()<deadline,'Native editor did not paint the selected color';time.sleep(.05)
        subprocess.run(['xdotool','key','--window',window,'6'],env=env,check=True)
        native_click(window,35,105,3);native_click(window,35,105)
        deadline=time.monotonic()+3
        while True:
            icy=capture(window,'native-editor-ice.png');r,g,b=icy.getpixel((px,py))
            if b>150 and g>130 and r>80 and b>=g>=r:break
            assert time.monotonic()<deadline,'Native editor ice pixels missing';time.sleep(.05)
        crop=(round(ox+21*scale),round(oy+101*scale),round(ox+49*scale),round(oy+110*scale))
        initial=icy.crop(crop).tobytes();deadline=time.monotonic()+3
        while capture(window,'native-editor-ice-animated.png').crop(crop).tobytes()==initial:
            assert time.monotonic()<deadline,'Native editor ice animation is frozen';time.sleep(.1)
        subprocess.run(['xdotool','key','--window',window,'7'],env=env,check=True)
        native_click(window,35,105,3);native_click(window,35,105)
        deadline=time.monotonic()+3
        while True:
            fire=capture(window,'native-editor-fire.png');r,g,b=fire.getpixel((px,py))
            if r>180 and r>g+30 and b<170:break
            assert time.monotonic()<deadline,'Native editor fire/glass pixels missing';time.sleep(.05)
        core_crop=(round(ox+22*scale),round(oy+101*scale),round(ox+49*scale),round(oy+109*scale))
        deadline=time.monotonic()+3
        def native_core_samples(image):return [image.getpixel((x,y)) for y in range(core_crop[1],core_crop[3]) for x in range(core_crop[0],core_crop[2])]
        while not fire_core_visible(native_core_samples(fire)):
            assert time.monotonic()<deadline,'Native renderer omitted the moving one-pixel core'
            time.sleep(.05);fire=capture(window,'native-editor-fire-core.png')
        initial=fire.crop(crop).tobytes();deadline=time.monotonic()+3
        while capture(window,'native-editor-fire-animated.png').crop(crop).tobytes()==initial:
            assert time.monotonic()<deadline,'Native editor fire/glass animation is frozen';time.sleep(.1)
        subprocess.run(['xdotool','key','--window',window,'1'],env=env,check=True);native_click(window,35,105,3);native_click(window,35,105)
        assert native_brick_matches(capture(window,'native-editor-painted.png'),1)
        subprocess.run(['xdotool','key','--window',window,'s'],env=env,check=True)
        deadline=time.monotonic()+3
        while True:
            stored=[p.read_bytes() for p in (root/'library/saves').iterdir()]
            if any(v[:4]==b'ARK3' and int.from_bytes(v[16:20],'little')==1 for v in stored):break
            assert time.monotonic()<deadline,'Native editor did not commit its pattern';time.sleep(.05)
        picker_title='GamePKG: com.gamepkg.arkanoo — Level pattern text'
        subprocess.run(['xdotool','key','--window',window,'i'],env=env,check=True)
        choose_native_file(env,picker_title,None);subprocess.run(['xdotool','windowfocus',window],env=env,check=True);time.sleep(.2)
        incoming=root/'native-import.txt';incoming.write_text('# Original ASCII pattern\n50******************\n')
        subprocess.run(['xdotool','key','--window',window,'i'],env=env,check=True);choose_native_file(env,picker_title,incoming)
        subprocess.run(['xdotool','windowfocus',window],env=env,check=True);deadline=time.monotonic()+5
        while True:
            imported=capture(window,'native-pattern-import.png')
            if native_brick_matches(imported,5):break
            assert time.monotonic()<deadline,'Native selected pattern did not appear';time.sleep(.05)
        assert incoming.read_text()=='# Original ASCII pattern\n50******************\n'
        outgoing=root/'native-export.txt';subprocess.run(['xdotool','key','--window',window,'x'],env=env,check=True);choose_native_file(env,picker_title,outgoing)
        deadline=time.monotonic()+5
        while not outgoing.exists():
            assert time.monotonic()<deadline,'Native Save dialog did not export the pattern';time.sleep(.05)
        assert outgoing.read_text().startswith('# Pattern: native-import\n') and '50******************' in outgoing.read_text()
        subprocess.run(['xdotool','windowactivate','--sync',window],env=env,check=True);time.sleep(.2)
        subprocess.run(['xdotool','key','--window',window,'F11'],env=env,check=True)
        deadline=time.monotonic()+5
        while '_NET_WM_STATE_FULLSCREEN' not in subprocess.run(['xprop','-id',window,'_NET_WM_STATE'],env=env,capture_output=True,text=True).stdout:
            assert time.monotonic()<deadline,'F11 did not enter actual fullscreen';time.sleep(.05)
        subprocess.run(['xdotool','key','--window',window,'F11'],env=env,check=True)
        deadline=time.monotonic()+5
        while '_NET_WM_STATE_FULLSCREEN' in subprocess.run(['xprop','-id',window,'_NET_WM_STATE'],env=env,capture_output=True,text=True).stdout:
            assert time.monotonic()<deadline,'F11 did not leave actual fullscreen';time.sleep(.05)
        subprocess.run(['xdotool','key','--window',window,'Escape','Up'],env=env,check=True);time.sleep(.2);expect_cursor(False)
        subprocess.run(['xdotool','key','--window',window,'Escape'],env=env,check=True);time.sleep(.2);expect_cursor(True)
        subprocess.run(['xdotool','key','--window',window,'q'],env=env,check=True)
        assert app.wait(timeout=10)==0,'Original Q-to-quit did not close cleanly'
        print('Native Arkanoo: gameplay/editor plus real file-picker import, cancellation and Save-dialog export passed.')

    finally:
        if app is not None:
            app.terminate()
            try:app.wait(timeout=10)
            except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
        if wm is not None:wm.terminate();wm.wait(timeout=10)
        display.terminate();display.wait(timeout=10)
    if args.skip_browser:raise SystemExit(0)
    # Both actual runtime profiles, not a hand-written replacement for the game.
    import shutil
    for component in [False,True]:
        game_id='com.gamepkg.arkanoo'
        if component:
            source=root/'component';shutil.copytree(PROJECT/'ports/arkanoo',source)
            manifest=source/'manifest.toml';manifest.write_text('runtime_profile = "component-v1"\n'+manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-component'))
            game_id+='-component'
            run('pack',source,'-o',root/'component.gamepkg','--key',key);run('install',root/'component.gamepkg','--trust-creator');run('build',game_id)
        out=root/('component-web' if component else 'core-web');run('web-export',game_id,'--output',out)
        server=ThreadingHTTPServer(('127.0.0.1',0),partial(Quiet,directory=str(out)));threading.Thread(target=server.serve_forever,daemon=True).start()
        downloads=root/('downloads-component' if component else 'downloads-core');downloads.mkdir()
        options=Options();options.binary_location='/usr/bin/chromium'
        options.add_experimental_option('prefs',{'download.default_directory':str(downloads),'download.prompt_for_download':False,'download.directory_upgrade':True})
        for arg in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--window-size=1200,1000']:options.add_argument(arg)
        browser=webdriver.Chrome(service=Service('/usr/bin/chromedriver'),options=options)
        try:
            browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20)
            wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
            browser.find_element(By.ID,'play').click();canvas=browser.find_element(By.ID,'game')
            wait.until(lambda b:canvas.get_attribute('data-state'))
            canvas.send_keys(Keys.ENTER);time.sleep(.15)
            def restart_menu():
                browser.find_element(By.ID,'restart').click();time.sleep(.15)
                canvas.send_keys(Keys.ENTER);time.sleep(.15)
            def canvas_click(x,y,button=1):
                # Click the visible image, including fullscreen aspect-ratio bars.
                rect=browser.execute_script('const c=arguments[0],r=c.getBoundingClientRect(),w=c.clientWidth,h=c.clientHeight,s=Math.min(w/640,h/480);return {x:r.left+c.clientLeft+(w-640*s)/2,y:r.top+c.clientTop+(h-480*s)/2,w:640*s,h:480*s};',canvas)
                bounds=canvas.rect
                action=ActionChains(browser).move_to_element_with_offset(canvas,rect['x']+x/640*rect['w']-(bounds['x']+bounds['width']/2),rect['y']+y/480*rect['h']-(bounds['y']+bounds['height']/2))
                (action.context_click() if button==3 else action.click()).perform()
            def canvas_drag(x):
                rect=browser.execute_script('const c=arguments[0],r=c.getBoundingClientRect(),w=c.clientWidth,h=c.clientHeight,s=Math.min(w/640,h/480);return {x:r.left+c.clientLeft+(w-640*s)/2,y:r.top+c.clientTop+(h-480*s)/2,w:640*s,h:480*s};',canvas)
                cy=math.ceil(rect['y']+245/480*rect['h'])
                cx=math.ceil(rect['x']+(319 if x==320 else 320)/640*rect['w'])
                tx=math.ceil(rect['x']+x/640*rect['w'])
                actions=ActionChains(browser)
                actions.w3c_actions.pointer_action.move_to_location(cx,cy)
                actions.click_and_hold()
                actions.w3c_actions.pointer_action.move_to_location(tx,cy)
                actions.pause(.15).release().perform()
            def saved_sfx():
                return browser.execute_script('const key=Object.keys(localStorage).find(key=>key.startsWith(arguments[0]));if(!key)return null;const a=JSON.parse(localStorage.getItem(key)),v=(a[12]|a[13]<<8)>>>0;return [(v>>>1)&255,!!(v&512)];','gamepkg:save:'+game_id+':')
            def expect_sfx(volume,muted):
                try:wait.until(lambda b:saved_sfx()==[volume,muted])
                except Exception:
                    canvas.screenshot(str(PROOF/'browser-sfx-failed.png'))
                    raise AssertionError(('Browser audio setting did not persist',saved_sfx(),volume,muted))
            def sfx_view():return browser.execute_script('return Array.from(arguments[0].getContext("2d").getImageData(290,219,60,31).data)',canvas)
            canvas_click(320,285)
            canvas_drag(295);expect_sfx(32,False)
            canvas_click(320,225);expect_sfx(32,True)
            ActionChains(browser).move_to_element_with_offset(canvas,0,0).perform()
            stamp=int(canvas.get_attribute('data-state'));wait.until(lambda b:int(canvas.get_attribute('data-state'))>stamp+2)
            muted_view=sfx_view()
            restart_menu();time.sleep(.15);canvas_click(320,285)
            wait.until(lambda b:sfx_view()==muted_view)
            canvas_click(320,225);canvas_drag(320);expect_sfx(64,False)
            canvas_click(320,270)
            wait.until(lambda b:b.execute_script('return document.fullscreenElement===document.querySelector("#game")'))
            canvas_click(320,270)
            wait.until(lambda b:b.execute_script('return document.fullscreenElement===null'))
            canvas_click(320,450)
            canvas_click(320,185);time.sleep(.15)

            def saved_gravity():
                return browser.execute_script('const key=Object.keys(localStorage).find(key=>key.startsWith(arguments[0]));return key?(JSON.parse(localStorage.getItem(key))[12]&1):null;','gamepkg:save:'+game_id+':')
            canvas.send_keys('v');wait.until(lambda b:saved_gravity()==1)
            canvas.send_keys('v');wait.until(lambda b:saved_gravity()==0)
            def pixels():return browser.execute_script('return Array.from(document.querySelector("#game").getContext("2d").getImageData(0,0,640,480).data)')
            def center(data):
                xs=[]
                for i in range(399*640,401*640):
                    r,g,b=data[i*4:i*4+3]
                    if r<50 and g>140 and b>190 and g-r>80 and b-g>20:xs.append(i%640)
                assert len(xs)>80,len(xs)
                return sum(xs)/len(xs)
            bounds=canvas.rect
            ActionChains(browser).move_to_element_with_offset(canvas,-bounds['width']*.25,0).perform()
            wait.until(lambda b:abs(center(pixels())-160)<5)
            left=center(pixels())
            ActionChains(browser).key_down(Keys.ARROW_RIGHT).pause(.3).key_up(Keys.ARROW_RIGHT).perform()
            assert center(pixels())>left+25
            ActionChains(browser).send_keys(Keys.ESCAPE).perform();time.sleep(.15)
            paused=pixels();time.sleep(.25);assert pixels()==paused,'pause changed pixels'
            ActionChains(browser).send_keys(Keys.ESCAPE).perform();time.sleep(2)
            assert pixels()!=paused
            canvas.screenshot(str(PROOF/('browser-component.png' if component else 'browser-core.png')))
            canvas.send_keys(Keys.ESCAPE)
            wait.until(lambda b:b.execute_script('return document.pointerLockElement===null'))
            restart_menu()
            wait.until(lambda b:original_menu_backdrop_matches(pixels()))
            restarted=pixels();time.sleep(.2);assert pixels()==restarted,'Restart retained the active run'
            canvas.send_keys('e');time.sleep(.15)
            canvas.send_keys('1')
            canvas_click(35,105)
            wait.until(lambda b:browser_brick_matches(pixels(),1))
            canvas.send_keys('6');canvas_click(35,105,3);canvas_click(35,105)
            def ice_visible(data):
                r,g,b=data[(105*640+35)*4:(105*640+35)*4+3]
                return b>150 and g>130 and r>80 and b>=g>=r
            wait.until(lambda b:ice_visible(pixels()))
            def ice_region(data):return [data[(y*640+x)*4+c] for y in range(101,110) for x in range(21,49) for c in range(4)]
            initial=ice_region(pixels());wait.until(lambda b:ice_region(pixels())!=initial)
            canvas.send_keys('7');canvas_click(35,105,3);canvas_click(35,105)
            def fire_visible(data):
                r,g,b=data[(105*640+35)*4:(105*640+35)*4+3]
                return r>180 and r>g+30 and b<170
            wait.until(lambda b:fire_visible(pixels()))
            def core_samples(data):return [tuple(data[(y*640+x)*4:(y*640+x)*4+3]) for y in range(101,109) for x in range(22,49)]
            wait.until(lambda b:fire_core_visible(core_samples(pixels())))
            initial=ice_region(pixels());wait.until(lambda b:ice_region(pixels())!=initial)
            canvas.send_keys('1');canvas_click(35,105,3);canvas_click(35,105);wait.until(lambda b:browser_brick_matches(pixels(),1))
            canvas.send_keys('n');canvas.send_keys(Keys.BACKSPACE)
            canvas_click(150,400) # Mouse toolbar remains active while editing the name.
            def saved_pattern():
                return browser.execute_script('const key=Object.keys(localStorage).find(key=>key.startsWith(arguments[0]));return key?JSON.parse(localStorage.getItem(key)):[];','gamepkg:save:'+game_id+':')
            wait.until(lambda b:saved_pattern()[:4]==[65,82,75,51] and saved_pattern()[16:20]==[1,0,0,0])
            restart_menu();time.sleep(.15);canvas.send_keys('e');canvas.send_keys('l');canvas.send_keys(Keys.ENTER)
            wait.until(lambda b:browser_brick_matches(pixels(),1))
            canvas.send_keys('t');time.sleep(.15);canvas.send_keys('q');time.sleep(.15)
            wait.until(lambda b:browser_brick_matches(pixels(),1))
            canvas.screenshot(str(PROOF/('browser-component-editor.png' if component else 'browser-core-editor.png')))
            canvas.send_keys('i');wait.until(lambda b:b.find_element(By.CSS_SELECTOR,'#user-files input[type=file]').is_displayed())
            browser.find_element(By.CSS_SELECTOR,'#user-files button').click();wait.until(lambda b:not b.find_element(By.ID,'user-files').is_displayed())
            assert browser_brick_matches(pixels(),1)
            incoming=root/('browser-component.txt' if component else 'browser-core.txt');incoming.write_text('# Original ASCII pattern\n50******************\n')
            canvas.send_keys('i');upload=wait.until(lambda b:b.find_element(By.CSS_SELECTOR,'#user-files input[type=file]'));upload.send_keys(str(incoming))
            wait.until(lambda b:browser_brick_matches(pixels(),5));assert incoming.read_text()=='# Original ASCII pattern\n50******************\n'
            browser.execute_script('Object.defineProperty(window,"showSaveFilePicker",{value:undefined,configurable:true})')
            canvas.send_keys('x');button=wait.until(lambda b:next((v for v in b.find_elements(By.CSS_SELECTOR,'#user-files button') if v.text.startswith('Save ')),None));button.click()
            outgoing=downloads/incoming.name
            wait.until(lambda b:outgoing.exists() and '50******************' in outgoing.read_text())
            assert outgoing.read_text().startswith('# Pattern: '+incoming.stem+'\n')
            canvas.screenshot(str(PROOF/('browser-component-files.png' if component else 'browser-core-files.png')))
            canvas.send_keys(Keys.F11)
            wait.until(lambda b:b.execute_script('return document.fullscreenElement===document.querySelector("#game")'))
            canvas.send_keys(Keys.F11)
            wait.until(lambda b:b.execute_script('return document.fullscreenElement===null'))
            restart_menu();time.sleep(.15)
            canvas.send_keys(Keys.ARROW_UP);time.sleep(.15)
            canvas.click()
            wait.until(lambda b:b.execute_script('return document.pointerLockElement===document.querySelector("#game")'))
            assert browser.execute_script('return document.querySelector("#game").style.cursor')=='none'
            canvas.send_keys(Keys.ESCAPE)
            wait.until(lambda b:b.execute_script('return document.pointerLockElement===null'))
            canvas.send_keys('q')
            wait.until(lambda b:'Game closed' in b.find_element(By.ID,'status').text)
            print(('Component' if component else 'Core')+' browser Arkanoo: gameplay/editor, selected-file import, cancellation and explicit download passed.')
        finally:browser.quit();server.shutdown();server.server_close()
