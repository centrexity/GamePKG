"""Actual ROM selection and controller input in the native Zelda3 player."""
from pathlib import Path
import argparse,csv,io,os,shutil,subprocess,tempfile,time
from PIL import Image,ImageChops
from gui_test_support import start_display,game_window,click_text,choose_native_file
def pixels(image):return image.get_flattened_data() if hasattr(image,"get_flattened_data") else image.getdata()
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('rom',type=Path);p.add_argument('--library',type=Path,required=True);p.add_argument('--wine-prefix',type=Path);args=p.parse_args()
PROOF=PROJECT/('target/zelda3-windows-proof' if args.wine_prefix else 'target/zelda3-proof');PROOF.mkdir(parents=True,exist_ok=True)
with tempfile.TemporaryDirectory(prefix='zelda3-gui-',dir=PROJECT/'target') as temporary:
    root=Path(temporary);shutil.copytree(args.library,root/'library',ignore=shutil.ignore_patterns('progress'))
    config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);env['DBUS_SESSION_BUS_ADDRESS']='unix:path=/tmp/gamepkg-no-session-bus'
    display=start_display(env,width=1800,height=1100)
    rom=args.rom
    command=[str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg-gui')]
    if args.wine_prefix:
        def windows(path):return 'Z:'+str(Path(path).resolve()).replace('/','\\')
        env.update(WINEPREFIX=str(args.wine_prefix.resolve()),WINEDEBUG='-all',GAMEPKG_HOME=windows(root/'library'),GAMEPKG_CONFIG=windows(config))
        env['GAMEPKG_ZELDA3_PYTHON']='intentionally-unavailable-decoder'

        rom=windows(args.rom)
        command=['wine',str(PROJECT/'target/x86_64-pc-windows-gnu/release/gamepkg-gui.exe')]
    app=subprocess.Popen(command,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        end=time.monotonic()+20
        while time.monotonic()<end:
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,text=True,capture_output=True,timeout=2)
            if found.returncode==0:break
            time.sleep(.1)
        assert found.returncode==0
        window=found.stdout.splitlines()[0]
        subprocess.run(['xdotool','windowsize',window,'1700','1000','windowfocus',window],env=env,check=True);time.sleep(.3)
        click_text(window,env,'com.gamepkg.zelda3',root,timeout=20);time.sleep(.3)
        def capture(name):
            path=PROOF/name;subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True,timeout=10);return path
        screen=capture('native-library.png')
        enlarged=root/'ocr.png';Image.open(screen).resize((5100,3000)).save(enlarged)
        tsv=subprocess.check_output(['tesseract',str(enlarged),'stdout','--psm','11','tsv'],stderr=subprocess.DEVNULL,text=True)
        words=[w for w in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip()]
        your=min([w for w in words if w['text']=='Your'],key=lambda w:int(w['top']))
        y=(int(your['top'])+int(your['height'])//2)//3
        subprocess.run(['xdotool','mousemove','--window',window,'350',str(y),'click','1'],env=env,check=True)
        choose_native_file(env,'Choose Your Zelda: A Link to the Past (USA) ROM',rom,windows=bool(args.wine_prefix))
        time.sleep(.3);capture('native-rom-selected.png')
        screen=capture('native-before-play.png')
        Image.open(screen).resize((5100,3000)).save(enlarged)
        tsv=subprocess.check_output(['tesseract',str(enlarged),'stdout','--psm','11','tsv'],stderr=subprocess.DEVNULL,text=True)
        words=[w for w in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip()]
        play=min([w for w in words if w['text']=='Play'],key=lambda w:int(w['top']))
        subprocess.run(['xdotool','mousemove','--window',window,str((int(play['left'])+int(play['width'])//2)//3),str((int(play['top'])+int(play['height'])//2)//3),'click','1'],env=env,check=True)
        window,pid=game_window(env,'com.gamepkg.zelda3');assert pid != app.pid
        subprocess.run(['xdotool','windowsize',window,'1700','1000','windowfocus',window],env=env,check=True)
        def key(value):subprocess.run(['xdotool','key','--window',window,value],env=env,check=True)
        def ocr(path):return subprocess.check_output(['tesseract',str(path),'stdout','--psm','11'],stderr=subprocess.DEVNULL,text=True).upper()
        def wait_text(label,name,limit=45):
            end=time.monotonic()+limit
            while time.monotonic()<end:
                path=capture(name)
                if label in ocr(path).replace('5','S'):return path
                time.sleep(.3)
            raise AssertionError('Zelda screen did not show '+label)
        # The source fixture's save was created through actual runtime controller input.
        subprocess.run(['xdotool','windowsize',window,'900','700'],env=env,check=True)
        def green_menu():
            im=Image.open(capture('native-file-menu.png')).convert('RGB')
            return sum(g>r+10 and g>b+10 for r,g,b in pixels(im.crop((0,90,900,700))))>40000
        end=time.monotonic()+90
        while time.monotonic()<end:
            if green_menu():break
            key('Return');time.sleep(1)
        assert green_menu(),'File-selection screen did not appear'
        time.sleep(5);menu=capture('native-file-menu.png')
        key('Return')
        end=time.monotonic()+45
        while time.monotonic()<end:
            im=Image.open(capture('native-opening.png')).convert('RGB')
            purple=sum(r>g+10 and b>r+10 for r,g,b in pixels(im.crop((0,90,900,700))))
            if purple>6000:break
            time.sleep(.5)
        assert purple>6000,'Saved slot did not load the opening room'
        for _ in range(30):key('z');time.sleep(2)
        before=capture('native-opening.png')
        subprocess.run(['xdotool','keydown','--window',window,'Right'],env=env,check=True);time.sleep(.5)
        subprocess.run(['xdotool','keyup','--window',window,'Right'],env=env,check=True)
        after=capture('native-right.png')
        difference=ImageChops.difference(Image.open(before).convert('RGB'),Image.open(after).convert('RGB'))
        assert sum(max(pixel)>30 for pixel in pixels(difference))>100,'No visible controller response'
        blobs=list((root/'library/saves').glob('*.blob'));assert len(blobs)==1 and blobs[0].stat().st_size==8192,'No scoped Zelda SRAM save'
        assert Image.open(menu).tobytes()!=Image.open(after).tobytes()
        print(('Windows under Wine' if args.wine_prefix else 'Native Linux')+' Zelda3: ROM selection, saved-slot loading, opening gameplay, held movement and SRAM save passed.')
    finally:
        app.terminate()
        try:app.wait(timeout=10)
        except subprocess.TimeoutExpired:app.kill();app.wait(timeout=5)
        display.terminate();display.wait(timeout=10)
