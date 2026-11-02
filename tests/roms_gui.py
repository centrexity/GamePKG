"""Play with signed named ROM requirements through the actual desktop library."""
import csv,io,json,os,shutil,subprocess,tempfile,time
from pathlib import Path
from PIL import Image
from gui_test_support import start_display,game_window,click_text,choose_native_file,close_game,wait_game_frame,click_primary_play
os.environ.setdefault('OMP_THREAD_LIMIT','1')
PROJECT=Path(__file__).resolve().parents[1]
for proof_name in ['roms-final.png','roms-first-game.png','roms-restarted-game.png','roms-picker-output.jsonl']:
    (PROJECT/'target'/proof_name).unlink(missing_ok=True)
with tempfile.TemporaryDirectory(prefix='gamepkg-roms-gui-') as directory:
    root=Path(directory);source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
    manifest=source/'manifest.toml'
    manifest.write_text(manifest.read_text()+'\n[[roms]]\nid="program"\ntitle="Program ROM"\n[[roms]]\nid="data"\ntitle="Data ROM"\n')
    (source/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,_:u64)->u64 {
        if !((gamepkg::rom_count()) == (2)) { loop {} };
        let mut program=[0u8;3];
        if !((gamepkg::rom_read_at(0,0,&mut program)) == (Some(3))) { loop {} };
        if !((&program) == (b"ROM")) { loop {} };
        let mut bytes=[0u8;4];
        if !((gamepkg::rom_read_at(1,0,&mut bytes)) == (Some(4))) { loop {} };
        if !((&bytes) == (b"DATA")) { loop {} }; gamepkg::draw_clear(0x00ff00ff);7u64<<32
    }''')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(root/'settings.toml'),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None);env['DBUS_SESSION_BUS_ADDRESS']='unix:path=/tmp/gamepkg-no-session-bus'
    # Keep native chooser preferences, recent files and caches inside this test.
    for variable,folder in [('XDG_CONFIG_HOME','desktop-config'),('XDG_DATA_HOME','desktop-data'),('XDG_CACHE_HOME','desktop-cache')]:
        location=root/folder;location.mkdir();env[variable]=str(location)
    env['GSETTINGS_BACKEND']='memory'
    # Relay the real native picker's bytes unchanged, retaining its exact result.
    # This diagnoses GTK selection failures without replacing the system dialog.
    picker_binary=shutil.which('zenity',path=env['PATH'])
    assert picker_binary,'The native Linux file picker must be installed'
    picker_log=root/'picker-output.jsonl';picker_bin=root/'picker-bin';picker_bin.mkdir()
    recorder=picker_bin/'zenity'
    recorder.write_text('#!/usr/bin/env python3\n'
        'import json,subprocess,sys\n'
        f'result=subprocess.run([{picker_binary!r},*sys.argv[1:]],capture_output=True)\n'
        f'with open({str(picker_log)!r},"a",encoding="utf-8") as log:\n'
        ' log.write(json.dumps(dict(args=sys.argv[1:],exit=result.returncode,stdout=result.stdout.hex(),stderr=result.stderr.hex()))+"\\n")\n'
        'sys.stdout.buffer.write(result.stdout);sys.stderr.buffer.write(result.stderr);sys.exit(result.returncode)\n')
    recorder.chmod(0o755);env['PATH']=str(picker_bin)+os.pathsep+env['PATH']
    def picker_receipts():
        # An in-flight append may end mid-record; only consume complete lines.
        text=picker_log.read_text() if picker_log.exists() else ''
        return [json.loads(line) for line in text.split('\n')[:-1] if line]
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
            words=[w for w in csv.DictReader(io.StringIO(tsv),delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip()]
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
        click_text(window,env,'com.gamepkg.minimal',root)
        def wait_for_selection(index,path):
            deadline=time.monotonic()+10
            while True:
                receipts=[record for record in picker_receipts() if '--file-selection' in record['args']]
                if len(receipts)>index:
                    returned=bytes.fromhex(receipts[index]['stdout']).decode().strip()
                    assert returned==str(path),f'Native picker returned {returned!r}, expected {str(path)!r}; receipt: {receipts[index]!r}'
                    selection_file=root/'library/rom-selections.json'
                    stored=json.loads(selection_file.read_text()) if selection_file.exists() else {}
                    expected=[str(a),str(b) if index else '']
                    if list(stored.values())==[expected]:return
                assert time.monotonic()<deadline,f'Named ROM selection was not persisted: slot={index}, path={str(path)!r}, receipts={receipts!r}'
                time.sleep(.1)
        for index,(label,path) in enumerate([('Program',a),('Data',b)]):
            click_text(window,env,'Choose ROM',root)
            choose_native_file(env,'Choose '+label+' ROM',path)
            wait_for_selection(index,path)
            if label=='Program':
                click(word('Play'));time.sleep(.3)
                found=subprocess.run(['xdotool','search','--onlyvisible','--name','^com.gamepkg.minimal$'],env=env,capture_output=True)
                assert found.returncode!=0,'Missing second ROM allowed a player to launch'
        click_primary_play(window,env,root)
        window,pid=game_window(env,'com.gamepkg.minimal');assert pid != app.pid
        shutil.copy2(wait_game_frame(window,env,root,(0,255,0)),PROJECT/'target/roms-first-game.png')
        close_game(window,env)
        app.terminate();app.wait(timeout=10)
        # A fresh direct launch must recover both slots without another picker.
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.minimal'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        window,_=game_window(env,'com.gamepkg.minimal')
        shutil.copy2(wait_game_frame(window,env,root,(0,255,0)),PROJECT/'target/roms-restarted-game.png')
        close_game(window,env)
        assert app.wait(timeout=10)==0,'Direct launch did not close normally'
        receipts=picker_receipts()
        assert sum('--file-selection' in receipt['args'] for receipt in receipts)==2,'Direct launch unexpectedly requested another ROM'
        print('Desktop named multi-ROM selection, missing-file gating and actual guest reads passed.')
        print('Fresh --play launch remembered both ROMs without selecting them again.')
    finally:
        if picker_log.exists():
            shutil.copy2(picker_log,PROJECT/'target/roms-picker-output.jsonl')
            for receipt in picker_receipts():
                print('Native picker receipt:',repr(bytes.fromhex(receipt['stdout'])), 'exit:',receipt['exit'],flush=True)
        if app.poll() is None:
            subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(PROJECT/'target/roms-final.png')],env=env,capture_output=True)
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
