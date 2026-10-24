"""Actual game-page recording controls and native Save dialog, private library."""
from pathlib import Path
import gzip,hashlib,os,struct,subprocess,tempfile,time,tomllib,zipfile
from gui_test_support import start_display,game_window,click_text,choose_native_file
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-recording-gui-') as temporary:
    root=Path(temporary);library=root/'library';key=root/'test.key';key.write_text('69'*32)
    config=root/'settings.toml';config.write_text('automatic_update_checks=false\nmemory_ceiling_mib=2048\n')
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config),OMP_THREAD_LIMIT='1',LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY',None)
    def cli(*args):
        result=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=30)
        assert result.returncode==0,(args,result.stdout,result.stderr)
    package=root/'test.gamepkg';cli('pack',PROJECT/'ports/cube2','-o',package,'--key',key);cli('install',package,'--trust-creator')
    with zipfile.ZipFile(package) as archive:manifest=tomllib.loads(archive.read('manifest.toml').decode())
    scope=f"GamePKG Cube 2 saved files v1\0{manifest['id']}\0{manifest['creator_key']}\0{manifest['save_schema']}"
    save=library/'saves'/(hashlib.sha256(scope.encode()).hexdigest()+'.cube2');save.parent.mkdir()
    movie=b'private exact movie fixture\0\xff';payload=b''
    for name,content in [('Movie.AVI',movie),('config.cfg',b'private preferences')]:
        encoded=name.encode();payload+=struct.pack('<4I',len(encoded),len(content),0,0)+encoded+content
    original=b'GPS2'+struct.pack('<3I',2,len(payload),0)+payload;save.write_bytes(original)
    server=start_display(env,1600,1050);app=None
    try:
        app=subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui')],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE)
        window,_=game_window(env,'GamePKG')
        subprocess.run(['xdotool','windowsize',window,'1450','980','windowfocus',window],env=env,check=True)
        time.sleep(.5);click_text(window,env,'Cube',root)
        click_text(window,env,'Load recordings',root)
        click_text(window,env,'Export movie',root)
        choose_native_file(env,'Save recording')
        subprocess.run(['xdotool','mousemove','--window',window,'400','400','click','--repeat','30','--delay','20','4'],env=env,check=True)
        time.sleep(.2)
        click_text(window,env,'Recording export cancelled.',root,click=False)
        click_text(window,env,'Export movie',root)
        output=root/'exported.avi';choose_native_file(env,'Save recording',output)
        deadline=time.monotonic()+5
        while not output.exists():
            assert time.monotonic()<deadline,'Native Save dialog did not export recording';time.sleep(.1)
        assert output.read_bytes()==movie and save.read_bytes()==original
        subprocess.run(['xdotool','mousemove','--window',window,'400','400','click','--repeat','30','--delay','20','4'],env=env,check=True)
        time.sleep(.2)
        click_text(window,env,'Recording saved to',root,click=False)
        click_text(window,env,'Refresh recordings',root)
        click_text(window,env,'Export movie',root,click=False)
        incoming=root/'incoming.dmo';incoming.write_bytes(gzip.compress(b'SAUERBRATEN_DEMO'+struct.pack('<5I',1,260,0,1,2)+b'\0\1'))
        click_text(window,env,'Import recording',root)
        choose_native_file(env,'Import recording')
        subprocess.run(['xdotool','mousemove','--window',window,'400','400','click','--repeat','30','--delay','20','4'],env=env,check=True);time.sleep(.2)
        click_text(window,env,'Recording import cancelled.',root,click=False)
        assert save.read_bytes()==original
        click_text(window,env,'Import recording',root);choose_native_file(env,'Import recording',incoming)
        subprocess.run(['xdotool','mousemove','--window',window,'400','400','click','--repeat','30','--delay','20','4'],env=env,check=True);time.sleep(.2)
        click_text(window,env,'2 saved recordings.',root,click=False)
        imported=save.read_bytes();assert imported!=original
        click_text(window,env,'Export demo',root)
        exported=root/'imported.dmo';choose_native_file(env,'Save recording',exported)
        deadline=time.monotonic()+5
        while not exported.exists():
            assert time.monotonic()<deadline,'Imported demo was not exported';time.sleep(.1)
        assert exported.read_bytes()==incoming.read_bytes() and save.read_bytes()==imported
        print('Recording game page: loaded signed scope, native Save cancellation and exact export, source preservation, refresh, native import cancellation and exact import/export passed.')
    except BaseException:
        proof=PROJECT/'target/recording-export-gui-failed.png'
        subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(proof)],env=env,capture_output=True)
        raise
    finally:
        if app and app.poll() is None:app.terminate();app.wait(timeout=10)
        server.terminate();server.wait(timeout=10)
