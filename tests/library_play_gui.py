"""Build and play through actual desktop library buttons, without startup shortcuts."""
from pathlib import Path
import csv, io, os, shutil, subprocess, tempfile, time
from gui_test_support import start_display,click_text,game_window
from PIL import Image
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-library-play-') as directory:
    root=Path(directory);library=root/'library'
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(root/'settings.toml'),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
    (root/'settings.toml').write_text('automatic_update_checks = false\n')
    def run(*args):
        return subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=60)
    key=root/'key';package=root/'game.gamepkg'
    run('keygen','-o',key);run('pack',project/'examples/good/collect-stars','-o',package,'--key',key)
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
            path=root/name
            subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
            shutil.copy2(path,project/'target'/name)
            original_text=subprocess.run(['tesseract',str(path),'stdout','--psm','6'],check=True,capture_output=True,text=True).stdout
            enlarged=root/'ocr.png'
            with Image.open(path) as image:
                image.resize((image.width*3,image.height*3)).save(enlarged)
            tsv=subprocess.run(['tesseract',str(enlarged),'stdout','--psm','6','tsv'],check=True,capture_output=True,text=True).stdout
            words=[word for word in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if word['text'].strip()]
            for word in words:
                for field in ['left','top','width','height']:word[field]=str(int(word[field])//3)
            row_image=root/'row-ocr.png'
            with Image.open(path) as image:
                region=image.crop((0,100,image.width,400))
                region.resize((region.width*3,region.height*3)).save(row_image)
            row_tsv=subprocess.run(['tesseract',str(row_image),'stdout','--psm','11','tsv'],check=True,capture_output=True,text=True).stdout
            row_words=[word for word in csv.DictReader(io.StringIO(row_tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if word['text'].strip()]
            for word in row_words:
                for field in ['left','top','width','height']:word[field]=str(int(word[field])//3)
                word['top']=str(int(word['top'])+100)
            controls=root/'controls.png'
            subprocess.run(['convert',str(path),'-colorspace','Gray','-negate','-threshold','55%','-resize','300%',str(controls)],check=True)
            control_tsv=subprocess.check_output(['tesseract',str(controls),'stdout','--psm','11','tsv'],stderr=subprocess.DEVNULL,text=True)
            control_words=[w for w in csv.DictReader(io.StringIO(control_tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip()]
            for word in control_words:
                for field in ['left','top','width','height']:word[field]=str(int(word[field])//3)
            words=control_words+row_words+words
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
        def enter_source(words, source):
            label=next(word for word in words if word['text']=='Or')
            subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
            subprocess.run(['xdotool','mousemove','--window',window,'100',str(int(label['top'])+int(label['height'])+12),'click','1'],env=env,check=True)
            time.sleep(.2)
            subprocess.run(['xdotool','key','ctrl+a','type','--clearmodifiers',str(source)],env=env,check=True)
            time.sleep(.2)
        text,words=wait_screen(lambda text:'No games yet' in text,'library-empty.png',20)
        enter_source(words,package)
        click_word(words,'Download')
        text,words=wait_screen(lambda text:'creator fingerprint:' in text.lower() and 'Trust this creator and install' in text,'library-inspected.png',20)
        assert not (library/'com.gamepkg.collect-stars.gamepkg').exists(), 'inspection installed a package'
        click_word(words,'Trust')
        text,words=wait_screen(lambda text:'com.gamepkg.collect-stars' in text and 'Not built' in text,'library-before-build.png')
        assert 'Rebuild' not in text, text
        click_word(words,'Build')
        text,words=wait_screen(lambda text:'Rebuild' in text,'library-after-build.png')
        assert len(list((library/'builds').glob('*/native-v1-*/game')))==1
        click_word(words,'More')
        text,words=wait_screen(lambda text:'Check integrity' in text,'game-more.png')
        click_word(words,'integrity')
        text,words=wait_screen(lambda text:'compiled game verified' in text,'library-integrity.png')
        library_window=window
        click_word(words,'Play')
        window,game_pid=game_window(env,'com.gamepkg.collect-stars')
        assert game_pid != app.pid
        text,words=wait_screen(lambda text:'Score:' in text,'library-click-play.png',20)
        assert 'com.gamepkg.collect-stars' in text, text
        time.sleep(1.5)
        subprocess.run(['xdotool','windowfocus','--sync',window],env=env,check=True)
        for direction in ['Left','Up']:
            subprocess.run(['xdotool','key','--clearmodifiers','--repeat','6','--delay','100',direction],env=env,check=True)
        time.sleep(.3)
        click_text(window,env,'Close game',root)
        window=library_window
        text,words=wait_screen(lambda text:'Rebuild' in text and 'Score:' not in text,'library-returned.png',20)
        assert 'Storage' in text and 'Played' in text, text
        assert 'First star' in text and 'Star collector' in text and 'Locked' in text and 'Unlocked' in text, text
        import json
        records=list((library/'progress').glob('*.json'))
        assert len(records)==1
        history=json.loads(records[0].read_text())
        assert history['played_ms'] >= 1000
        assert set(history['unlocked']) == {'first_star'}, history
        click_word(words,'Rebuild')
        wait_screen(lambda text:'Native build ready to play.' in text,'library-rebuilt.png')
        assert len(list((library/'builds').glob('*/native-v1-*/game')))==1
        text,words=read_screen('library-before-catalog.png')
        click_word(words,'Back')
        text,words=read_screen('library-catalog-source.png')
        catalog=root/'games.gamepkglist'
        catalog.write_text("format_version = 1\n[[games]]\nid = 'com.example.offline'\ntitle = 'Offline Catalog Game'\nversion = '2.0.0'\ndownload_url = 'https://unreachable.invalid/game.gamepkg'\nrequires_rom = true\n")
        enter_source(words,catalog)
        click_word(words,'Import')
        text,words=wait_screen(lambda text:'Not downloaded' in text and 'No game packages downloaded' in text,'library-catalog-imported.png',20)
        click_word(words,'Offline')
        text,words=wait_screen(lambda text:'Back to library' in text and 'Catalog information is unverified' in text,'game-catalog-page.png',20)
        assert 'Requires your own ROM' in text, text
        assert 'com.gamepkg.collect-stars' not in text and 'Import .gamepkglist' not in text, text
        click_word(words,'Back')
        text,words=wait_screen(lambda text:'Offline Catalog Game' in text and 'com.gamepkg.collect-stars' in text,'game-page-return-to-library.png',20)
        click_text(window,env,'com.gamepkg.collect-stars',root)
        text,words=wait_screen(lambda text:'About this game' in text and 'Rebuild' in text,'game-installed-page.png',20)
        assert 'Offline Catalog Game' not in text and 'Download / inspect' not in text, text

        assert 'com.example.offline' in (library/'catalog.gamepkglist').read_text()
        assert list(library.glob('*.gamepkg')) == [library/'com.gamepkg.collect-stars.gamepkg']
        assert not (library/'com.example.offline.gamepkg').exists()
        click_text(window,env,'Play',root)
        game,pid=game_window(env,'com.gamepkg.collect-stars');assert pid != app.pid
        app.terminate();app.wait(timeout=10)
        time.sleep(.5)
        os.kill(pid,0)
        assert subprocess.check_output(['xdotool','getwindowname',game],env=env,text=True).strip()=='com.gamepkg.collect-stars'
        click_text(game,env,'Close game',root)
        print('Desktop workflow passed: inspect, creator approval, install, Build, Play, return, Rebuild, and offline catalog import without downloading packages.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
