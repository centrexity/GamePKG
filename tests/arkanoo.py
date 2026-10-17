"""Signed Arkanoo port: isolated builds, native keyboard play and browser export."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import os, shutil, subprocess, tempfile, threading, time
from PIL import Image
from gui_test_support import start_display
import argparse
parser=argparse.ArgumentParser()
parser.add_argument('--skip-browser',action='store_true',help='Run native checks on hosts without Chromium/Selenium')
args=parser.parse_args()
if not args.skip_browser:
    from selenium import webdriver
    from selenium.webdriver.chrome.options import Options
    from selenium.webdriver.chrome.service import Service
    from selenium.webdriver.common.by import By
    from selenium.webdriver.common.action_chains import ActionChains
    from selenium.webdriver.common.keys import Keys
    from selenium.webdriver.support.ui import WebDriverWait
PROJECT=Path(__file__).resolve().parents[1];CLI=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'
PROOF=PROJECT/'target/arkanoo-proof';PROOF.mkdir(parents=True,exist_ok=True)
class Quiet(SimpleHTTPRequestHandler):
    def log_message(self,*args):pass
with tempfile.TemporaryDirectory(prefix='gamepkg-arkanoo-',dir=PROJECT/'target') as temp:
    root=Path(temp);config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
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
    # Trigger the production portal at its original speed threshold without
    # depending on a long random play session. Only fixture setup is replaced.
    modes=root/'modes';shutil.copytree(PROJECT/'ports/arkanoo',modes)
    mode_manifest=modes/'manifest.toml';mode_manifest.write_text(mode_manifest.read_text().replace('com.gamepkg.arkanoo','com.gamepkg.arkanoo-modes'))
    (modes/'source/src/lib.rs').write_text('''mod physics;mod render;use std::sync::Mutex;
static GAME:Mutex<Option<physics::Game>>=Mutex::new(None);
pub fn game_tick(input:u32,state:u64)->u64 {
 if let Ok(mut slot)=GAME.lock(){
  if slot.is_none(){let mut game=physics::Game::new();game.mode=1;game.blocks=[0;200];game.blocks[0]=1;game.balls[0].attached=false;game.balls[0].vx=30.0;game.balls[0].vy=0.0;*slot=Some(game);}
  if let Some(game)=slot.as_mut(){
   if input&4096!=0 {game.portal=false;game.gravity=false;game.max_speed=0.0;game.mode=1;game.particles.clear();game.balls.clear();
    game.balls.push(physics::Ball{x:100.0,y:250.0,vx:1.0,vy:2.0,spin:0.0,attached:false,launch_timer:0});
    game.balls.push(physics::Ball{x:104.0,y:250.0,vx:-1.0,vy:-2.0,spin:0.0,attached:false,launch_timer:0});
   }
   if input&1024!=0 {game.portal=false;game.gravity=false;game.max_speed=0.0;game.mode=1;game.blocks=[0;200];game.blocks[0]=1;game.balls.clear();game.balls.push(physics::Ball{x:34.0,y:114.0,vx:0.0,vy:-4.0,spin:0.0,attached:false,launch_timer:0});}
   if input&2048!=0 {game.portal=false;game.gravity=false;game.max_speed=0.0;game.mode=1;game.blocks=[0;200];game.blocks[0]=1;game.balls.clear();
    game.balls.push(physics::Ball{x:300.0,y:396.0,vx:1.0,vy:2.0,spin:0.0,attached:false,launch_timer:0});
    game.balls.push(physics::Ball{x:304.0,y:396.0,vx:-1.0,vy:-2.0,spin:0.0,attached:false,launch_timer:0});
   }
   if input&8192!=0 {*game=physics::Game::new();game.mode=1;}
   if input&16384!=0 {*game=physics::Game::new();game.mode=1;game.level=9;game.width=90.0;game.long=300;game.ghost=600;game.ammo=1;game.score=100;game.balls.clear();}
   if input&32768!=0 {*game=physics::Game::new();game.mode=1;game.level=999;game.blocks=[0;200];game.score=10000004;}
   if input&65536!=0 {*game=physics::Game::new();game.mode=1;game.blocks=[0;200];game.set_block(0,1);game.set_block(1,3);game.set_block(2,2);game.set_block(199,0);game.balls.clear();game.balls.push(physics::Ball{x:34.0,y:114.0,vx:0.0,vy:-4.0,spin:0.0,attached:false,launch_timer:0});}
   if input&262144!=0 {game.balls.clear();game.balls.push(physics::Ball{x:34.0,y:114.0,vx:0.0,vy:-4.0,spin:0.0,attached:false,launch_timer:0});}
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
   return (u64::from(game.score)<<32)|u64::from(game.portal)|(u64::from(game.gravity)<<1)|(u64::from(game.mode==3)<<2)|(u64::from(contact)<<3)|(u64::from(brick)<<4)|(u64::from(ordered)<<5)|(u64::from(waiting)<<6)|(u64::from(stationary)<<7)|(u64::from(game.mode==5)<<10)|(u64::from(preserved)<<11)|(u64::from(ice)<<12)|(u64::from(blast)<<13)|(u64::from(game.lives)<<8)|(u64::from(game.level)<<16);
  }
 }state
}''')
    run('pack',modes,'-o',root/'modes.gamepkg','--key',key);run('install',root/'modes.gamepkg','--trust-creator');run('build','com.gamepkg.arkanoo-modes')
    probe_env=dict(env,GAMEPKG_ARKANOO_TEST_ROOT=str(root/'library'))
    subprocess.run(['cargo','test','--test','game_arkanoo_runtime','--','--ignored'],env=probe_env,check=True,timeout=120)
    run('build','com.gamepkg.arkanoo','--target','native')
    run('build','com.gamepkg.arkanoo-modes','--target','native')
    run('integrity-check','com.gamepkg.arkanoo')
    subprocess.run(['cargo','test','--test','game_arkanoo_runtime','--','--ignored'],env=probe_env,check=True,timeout=120)
    # Real native window: locate the cyan paddle, hold movement, start, pause.
    display=start_display(env,width=1400,height=900);app=None
    def capture(window,name):
        path=PROOF/name
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10)
        return Image.open(path).convert('RGB')
    def paddle(image):
        points=[(x,y) for y in range(image.height//2,image.height-20) for x in range(image.width)
                if image.getpixel((x,y))==(99,220,255)]
        assert len(points)>100,len(points)
        return sum(x for x,y in points)/len(points)
    try:
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
        subprocess.run(['xdotool','key','--window',window,'Up'],env=env,check=True);time.sleep(.2)
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
        subprocess.run(['xdotool','key','--window',window,'Down'],env=env,check=True);time.sleep(.2)
        capture(window,'native-paused.png')
        print('Native Arkanoo: visible campaign, start, held-key paddle movement and pause passed.')
    finally:
        if app is not None:
            app.terminate()
            try:app.wait(timeout=10)
            except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
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
        options=Options();options.binary_location='/usr/bin/chromium'
        for arg in ['--headless=new','--no-sandbox','--disable-dev-shm-usage','--enable-unsafe-swiftshader','--window-size=1200,1000']:options.add_argument(arg)
        browser=webdriver.Chrome(service=Service('/usr/bin/chromedriver'),options=options)
        try:
            browser.get(f'http://127.0.0.1:{server.server_port}/');wait=WebDriverWait(browser,20)
            wait.until(lambda b:b.find_element(By.ID,'status').text=='Ready to play')
            browser.find_element(By.ID,'play').click();canvas=browser.find_element(By.ID,'game')
            wait.until(lambda b:canvas.get_attribute('data-state'))
            ActionChains(browser).click(canvas).send_keys(Keys.ARROW_UP).perform();time.sleep(.15)
            def saved_gravity():
                return browser.execute_script('const key=Object.keys(localStorage).find(key=>key.startsWith(arguments[0]));return key?JSON.parse(localStorage.getItem(key))[12]:null;','gamepkg:save:'+game_id+':')
            canvas.send_keys('v');wait.until(lambda b:saved_gravity()==1)
            canvas.send_keys('v');wait.until(lambda b:saved_gravity()==0)
            def pixels():return browser.execute_script('return Array.from(document.querySelector("#game").getContext("2d").getImageData(0,0,640,480).data)')
            def center(data):
                xs=[i%640 for i in range(395*640,406*640) if data[i*4:i*4+3]==[99,220,255]]
                assert len(xs)>100
                return sum(xs)/len(xs)
            left=center(pixels())
            ActionChains(browser).key_down(Keys.ARROW_RIGHT).pause(.3).key_up(Keys.ARROW_RIGHT).perform()
            assert center(pixels())>left+25
            ActionChains(browser).send_keys(Keys.ARROW_DOWN).perform();time.sleep(.15)
            paused=pixels();time.sleep(.25);assert pixels()==paused,'pause changed pixels'
            ActionChains(browser).send_keys(Keys.ARROW_DOWN).perform();time.sleep(2)
            assert pixels()!=paused
            canvas.screenshot(str(PROOF/('browser-component.png' if component else 'browser-core.png')))
            browser.find_element(By.ID,'stop').click()
            assert 'Stopped' in browser.find_element(By.ID,'status').text
            print(('Component' if component else 'Core')+' browser Arkanoo: held movement, pause/resume and export passed.')
        finally:browser.quit();server.shutdown();server.server_close()
