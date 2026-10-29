"""Verify actual library rows, rejection notices, and recovery from an unreadable folder."""
from pathlib import Path
import csv, io, os, shutil, subprocess, tempfile, time
from gui_test_support import start_display, click_text
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-library-gui-') as directory:
    root=Path(directory);library=root/'library'
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(root/'settings.toml'),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
    (root/'settings.toml').write_text('automatic_update_checks = false\n')
    def run(*args):
        return subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=60)
    key=root/'key';package=root/'game.gamepkg'
    source=root/'source';shutil.copytree(project/'examples/minimal',source)
    manifest=source/'manifest.toml';manifest.write_text('title = "Recovery Game"\n'+manifest.read_text())
    run('keygen','-o',key);run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator')
    catalog=root/'games.gamepkglist'
    catalog.write_text('format_version=1\n[[games]]\nid="com.example.pending-recovery"\ntitle="Pending Recovery"\ndownload_url="https://example.invalid/pending.gamepkg"\n')
    run('import-list',catalog)
    (library/'broken.gamepkg').write_bytes(b'invalid archive')
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
            enlarged=root/'ocr.png'
            subprocess.run(['convert',str(path),'-resize','300%',str(enlarged)],env=env,check=True)
            tsv=subprocess.run(['tesseract',str(enlarged),'stdout','--psm','11','tsv'],check=True,capture_output=True,text=True).stdout
            words=[word for word in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if word['text'].strip()]
            for word in words:
                for field in ['left','top','width','height']:word[field]=str(int(word[field])//3)
            return ' '.join(word['text'] for word in words), words
        text,words=read_screen('library-rejected.png')
        assert 'Recovery Game' in text and '0.1.0' in text and 'Not built' in text, text
        assert 'Unavailable package:' in text and 'broken.gamepkg' in text, text
        assert 'Pending Recovery' in text and 'Not downloaded' in text, text
        refresh=next(word for word in words if word['text']=='Refresh')
        library.rename(root/'saved-library');library.write_text('not a directory')
        subprocess.run(['xdotool','mousemove','--window',window,str(int(refresh['left'])+int(refresh['width'])//2),str(int(refresh['top'])+int(refresh['height'])//2),'click','1'],env=env,check=True)
        time.sleep(.5)
        text,_=read_screen('library-unreadable.png')
        assert 'Unable to read installed library' in text, text
        assert 'Recovery Game' not in text and 'com.gamepkg.minimal' not in text and 'broken.gamepkg' not in text, 'stale library rows remained: '+text
        assert 'Pending Recovery' not in text and 'com.example.pending-recovery' not in text, 'stale catalog rows remained: '+text
        library.unlink();(root/'saved-library').rename(library)
        click_text(window,env,'Refresh library',root)
        time.sleep(.5)
        text,_=read_screen('library-recovered.png')
        assert 'Recovery Game' in text and '0.1.0' in text and 'Not built' in text, text
        assert 'Unavailable package:' in text and 'broken.gamepkg' in text, text
        assert 'Pending Recovery' in text and 'Not downloaded' in text, text
        assert 'Unable to read installed library' not in text, text
        print('Library GUI passed: valid rows and rejected notices coexist; unreadable folders clear stale entries; restoring the folder recovers the library.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
