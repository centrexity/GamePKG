"""Play with signed named ROM requirements through the actual desktop library."""
import csv,io,os,shutil,subprocess,tempfile,time
from pathlib import Path
from PIL import Image
from gui_test_support import start_display
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-roms-gui-') as directory:
    root=Path(directory);source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
    manifest=source/'manifest.toml'
    manifest.write_text(manifest.read_text()+'\n[[roms]]\nid="program"\ntitle="Program ROM"\n[[roms]]\nid="data"\ntitle="Data ROM"\n')
    (source/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,_:u64)->u64 {
        assert_eq!(gamepkg::rom_count(),2);
        let mut bytes=[0u8;4];
        assert_eq!(gamepkg::rom_read_at(1,0,&mut bytes),Some(4));
        assert_eq!(&bytes,b"DATA"); 7u64<<32
    }''')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
    (root/'settings.toml').write_text('automatic_update_checks=false\n')
    def run(*args):subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,check=True,capture_output=True,text=True,timeout=180)
    key=root/'key';package=root/'game.gamepkg';run('keygen','-o',key);run('pack',source,'-o',package,'--key',key);run('install',package,'--trust-creator');run('build','com.gamepkg.minimal')
    a=root/'program.rom';a.write_bytes(b'ROM');b=root/'data.rom';b.write_bytes(b'DATA')
    display=start_display(env,1600,1000)
    app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
            if found.returncode==0:break
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            time.sleep(.1)
        else:raise AssertionError('GUI did not open')
        window=found.stdout.splitlines()[0]
        subprocess.run(['xdotool','windowsize',window,'1500','950','windowfocus',window],env=env,check=True);time.sleep(.3)
        def screen():
            path=root/'screen.png';subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
            shutil.copy2(path,PROJECT/'target/roms-gui.png')
            with Image.open(path) as image:image.resize((image.width*3,image.height*3)).save(root/'ocr.png')
            tsv=subprocess.run(['tesseract',str(root/'ocr.png'),'stdout','--psm','11','tsv'],check=True,capture_output=True,text=True).stdout
            words=[w for w in csv.DictReader(io.StringIO(tsv),delimiter='\t') if w['text'].strip()]
            for w in words:
                for field in ['left','top','width','height']:w[field]=str(int(w[field])//3)
            return ' '.join(w['text'] for w in words),words
        def word(label):
            end=time.monotonic()+30
            while True:
                text,words=screen()
                matches=[w for w in words if w['text'].strip('[]()|')==label]
                if matches:return min(matches,key=lambda w:int(w['top']))
                assert time.monotonic()<end,text
                time.sleep(.2)
        def click(w):subprocess.run(['xdotool','mousemove','--window',window,str(int(w['left'])+int(w['width'])//2),str(int(w['top'])+int(w['height'])//2),'click','1'],env=env,check=True);time.sleep(.2)
        click(word('Details'))
        for label,path in [('Program',a),('Data',b)]:
            w=word(label)
            subprocess.run(['xdotool','mousemove','--window',window,'250',str(int(w['top'])+int(w['height'])//2),'click','1','type','--clearmodifiers',str(path)],env=env,check=True)
            time.sleep(.2)
            if label=='Program':
                click(word('Play'));text,_=screen();assert 'Score:' not in text,'missing second ROM allowed playback'
        click(word('Play'))
        end=time.monotonic()+20
        while True:
            text,_=screen()
            if 'Score: 7' in text:break
            assert time.monotonic()<end,text
            time.sleep(.2)
        assert 'Game stopped' not in text,text
        print('Desktop named multi-ROM selection, missing-file gating and actual guest reads passed.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
