"""Desktop store imports require review, survive source removal, and allow cancellation."""
from pathlib import Path
import csv, io, os, shutil, subprocess, tempfile, time
from gui_test_support import start_display
project = Path(__file__).resolve().parents[1]
cli = project/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-store-gui-') as directory:
    root = Path(directory)
    source = root/'source'
    shutil.copytree(project/'examples/minimal', source)
    env = dict(os.environ, GAMEPKG_HOME=str(root/'library'), GAMEPKG_CONFIG=str(root/'settings.toml'), LIBGL_ALWAYS_SOFTWARE='1')
    env.pop('WAYLAND_DISPLAY', None)
    (root/'settings.toml').write_text('automatic_update_checks = false\n')
    def run(*args):
        return subprocess.run([str(cli), *map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=60)
    key=root/'creator.key';package=root/'game.gamepkg';bundle=root/'games.gamepkgstore'
    run('keygen','-o',key)
    run('pack',source,'-o',package,'--key',key,'--shared-assets')
    run('store-export',package,'-o',bundle)
    display=start_display(env,1200,900)
    app=subprocess.Popen([str(project/'target/release/gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
            if found.returncode==0:break
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            time.sleep(.1)
        else:raise AssertionError('GUI did not open')
        window=found.stdout.splitlines()[0]
        subprocess.run(['xdotool','windowsize',window,'1100','850'],env=env,check=True);time.sleep(.3)
        def click(x,y):
            subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'click','1'],env=env,check=True)
            time.sleep(.3)
        def screen(name):
            path=root/name
            subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
            shutil.copy2(path,project/'target'/name)
            result=subprocess.run(['tesseract',str(path),'stdout','--psm','6','tsv'],check=True,capture_output=True,text=True).stdout
            return [w for w in csv.DictReader(io.StringIO(result),delimiter='\t') if w['text'].strip()]
        def wait_word(label,name):
            deadline=time.monotonic()+30
            while True:
                if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
                words=screen(name)
                found=next((w for w in words if w['text'].strip('[]()|').casefold()==label.casefold()),None)
                if found:return found
                assert time.monotonic()<deadline,'Missing control '+label+': '+' '.join(w['text'] for w in words)
                time.sleep(.2)
        def click_label(label,name):
            w=wait_word(label,name)
            click(int(w['left'])+int(w['width'])//2,int(w['top'])+int(w['height'])//2)
        subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
        label=wait_word('Local','store-before.png')
        click(100,int(label['top'])+int(label['height'])+12)
        subprocess.run(['xdotool','type','--clearmodifiers',str(bundle)],env=env,check=True)
        click_label('Inspect','store-before.png')
        wait_word('Cancel','store-review.png')
        assert not (root/'library/com.gamepkg.minimal.gamepkg').exists(), 'inspection installed a game without approval'
        click_label('Cancel','store-review.png')
        assert not (root/'library/com.gamepkg.minimal.gamepkg').exists(), 'cancellation installed a game'
        click_label('Inspect','store-cancelled.png')
        wait_word('Cancel','store-review.png')
        bundle.unlink()
        click_label('Trust','store-review.png')
        installed=root/'library/com.gamepkg.minimal.gamepkg'
        for _ in range(100):
            if installed.exists():break
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            time.sleep(.1)
        assert installed.exists(), 'approved store import did not install the private verified snapshot'
        assert 'com.gamepkg.minimal' in run('list').stdout
        assert list((root/'library/asset-pool').glob('*.blob')), 'shared assets were not imported'
        print('GUI store import passed: inspection, cancellation, explicit approval, private snapshot, and shared assets.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
