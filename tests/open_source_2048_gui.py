"""Verify actual native 2048 rendering and keyboard gameplay."""
from gui_test_support import start_display, click_text
from pathlib import Path
from PIL import Image, ImageChops
import os, subprocess, tempfile, time
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-2048-gui-') as temporary:
    root=Path(temporary);env=dict(os.environ,LIBGL_ALWAYS_SOFTWARE='1',GAMEPKG_CONFIG=str(root/'settings.toml'),GAMEPKG_HOME=str(root/'library'));env.pop('WAYLAND_DISPLAY',None)
    (root/'settings.toml').write_text('automatic_update_checks=false\n')
    def run(*args):
        result=subprocess.run([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=120)
        assert result.returncode==0,result.stderr
    key=root/'key';key.write_text('55'*32)
    run('pack',PROJECT/'ports/2048','-o',root/'2048.gamepkg','--key',key)
    run('install',root/'2048.gamepkg','--trust-creator');run('build','com.gamepkg.2048')
    display=start_display(env, width=1900, height=1000)
    app=None
    try:
        app=subprocess.Popen([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg-gui'),'--play','com.gamepkg.2048'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True,timeout=2)
            if found.returncode==0:break
            time.sleep(.1)
        assert found.returncode==0
        window=found.stdout.splitlines()[0]
        subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
        proof=PROJECT/'target/2048-native-proof';proof.mkdir(parents=True,exist_ok=True)
        before=proof/'before.png';after=proof/'after-right.png'
        time.sleep(.5)
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(before)],env=env,check=True,timeout=10)
        subprocess.run(['xdotool','key','--window',window,'Right'],env=env,check=True)
        time.sleep(.4)
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(after)],env=env,check=True,timeout=10)
        first=Image.open(before).convert('RGB');second=Image.open(after).convert('RGB')
        tiles=sum(1 for r,g,b in second.getdata() if r>180 and g>120 and b>80 and r>=g>=b)
        assert tiles>10000,tiles
        changed=sum(1 for pixel in ImageChops.difference(first,second).getdata() if max(pixel)>30)
        assert changed>1000,changed
        app.terminate();app.wait(timeout=10)
        app=subprocess.Popen([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True,timeout=2)
            if found.returncode==0:break
            time.sleep(.1)
        assert found.returncode==0
        window=found.stdout.splitlines()[0]
        subprocess.run(['xdotool','windowsize',window,'1850','950'],env=env,check=True)
        subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
        click_text(window,env,'Export',root,timeout=20)
        deadline=time.monotonic()+15
        while time.monotonic()<deadline:
            exported=list((root/'library/web-exports').glob('*/index.html'))
            if exported:break
            time.sleep(.1)
        assert len(exported)==1
        assert (exported[0].parent/'source.gamepkg').is_file()
        print('Native 2048 GUI passed: visible tiles, keyboard board changes and the web export button.')
    finally:
        if app is not None:
            app.terminate()
            try:app.wait(timeout=10)
            except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
        display.terminate();display.wait(timeout=10)
