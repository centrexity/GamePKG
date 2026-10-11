"""Build and play through actual desktop library buttons, without startup shortcuts."""
from pathlib import Path
import csv, io, os, subprocess, tempfile, time
from gui_test_support import start_display
from PIL import Image
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-library-play-') as directory:
    root=Path(directory);library=root/'library'
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(root/'settings.toml'),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
    (root/'settings.toml').write_text('automatic_update_checks = false\n')
    def run(*args):
        return subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=60)
    key=root/'key';package=root/'game.gamepkg'
    run('keygen','-o',key);run('pack',project/'examples/minimal','-o',package,'--key',key);run('install',package,'--trust-creator')
    display=start_display(env,1600,1000)
    app=subprocess.Popen([str(project/'target/release/gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
            if found.returncode==0:break
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            time.sleep(.1)
        else:raise AssertionError('GUI did not open')
        window=found.stdout.splitlines()[0]
        subprocess.run(['xdotool','windowsize',window,'1500','950'],env=env,check=True);time.sleep(.5)
        def read_screen(name):
            path=project/'target'/name
            subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
            original_text=subprocess.run(['tesseract',str(path),'stdout','--psm','6'],check=True,capture_output=True,text=True).stdout
            enlarged=root/'ocr.png'
            with Image.open(path) as image:
                image.resize((image.width*3,image.height*3)).save(enlarged)
            tsv=subprocess.run(['tesseract',str(enlarged),'stdout','--psm','6','tsv'],check=True,capture_output=True,text=True).stdout
            words=[word for word in csv.DictReader(io.StringIO(tsv),delimiter='\t') if word['text'].strip()]
            for word in words:
                for field in ['left','top','width','height']:word[field]=str(int(word[field])//3)
            return original_text+' '+' '.join(word['text'] for word in words), words
        def wait_screen(predicate, name, seconds=90):
            deadline=time.monotonic()+seconds
            while True:
                if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
                text,words=read_screen(name)
                if predicate(text):return text,words
                assert time.monotonic()<deadline, text
                time.sleep(.2)
        def click_word(words, label):
            word=next(word for word in words if word['text'].strip('[](){}|')==label or (label=='Back' and word['text'].strip('[](){}|').startswith('Back')))
            subprocess.run(['xdotool','mousemove','--window',window,str(int(word['left'])+int(word['width'])//2),str(int(word['top'])+int(word['height'])//2),'click','1'],env=env,check=True)
        text,words=wait_screen(lambda text:'com.gamepkg.minimal' in text and 'Not built' in text,'library-before-build.png')
        assert 'Rebuild' not in text, text
        click_word(words,'Build')
        text,words=wait_screen(lambda text:'Rebuild' in text,'library-after-build.png')
        assert len(list((library/'builds').glob('*/game.wasm')))==1
        click_word(words,'Play')
        text,words=wait_screen(lambda text:'Score:' in text,'library-click-play.png',20)
        assert 'com.gamepkg.minimal' in text, text
        click_word(words,'Back')
        text,words=wait_screen(lambda text:'Rebuild' in text and 'Score:' not in text,'library-returned.png',20)
        click_word(words,'Rebuild')
        wait_screen(lambda text:'Built com.gamepkg.minimal' in text,'library-rebuilt.png')
        assert len(list((library/'builds').glob('*/game.wasm')))==1
        text,words=read_screen('library-before-catalog.png')
        catalog=root/'games.gamepkglist'
        catalog.write_text("format_version = 1\n[[games]]\nid = 'com.example.offline'\ntitle = 'Offline Catalog Game'\nversion = '2.0.0'\ndownload_url = 'https://unreachable.invalid/game.gamepkg'\nrequires_rom = true\n")
        label=next(word for word in words if word['text']=='Local')
        subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
        subprocess.run(['xdotool','mousemove','--window',window,'100',str(int(label['top'])+int(label['height'])+12),'click','1'],env=env,check=True)
        time.sleep(.2)
        subprocess.run(['xdotool','type','--clearmodifiers',str(catalog)],env=env,check=True)
        time.sleep(.2)
        click_word(words,'Import')
        wait_screen(lambda text:'Not downloaded' in text and 'No game packages downloaded' in text,'library-catalog-imported.png',20)
        assert 'com.example.offline' in (library/'catalog.gamepkglist').read_text()
        assert list(library.glob('*.gamepkg')) == [library/'com.gamepkg.minimal.gamepkg']
        assert not (library/'com.example.offline.gamepkg').exists()
        print('Desktop workflow passed: Build, Play, return, Rebuild, and offline catalog import without downloading packages.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
