"""Exercise the real Settings updater, picker, checksum failure and restart.

Only HTTPS transport is replaced with local release fixtures. No user's library
or installed copy is used. The updated launcher writes a restart receipt.
"""
from pathlib import Path
import hashlib, io, json, os, subprocess, tarfile, tempfile, time
from gui_test_support import start_display, click_text, choose_native_file, game_window
project = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-app-update-gui-') as directory:
    root = Path(directory)
    transport = root/'transport'; transport.mkdir()
    destination = root/'installed'; destination.mkdir()
    library = root/'library'; library.mkdir(); (library/'keep').write_text('keep my library')
    config = root/'settings.toml'; config.write_text('automatic_update_checks = false\nrenderer = "opengl"\n')
    version='99.0.0'; name=f'gamepkg-{version}-linux-x86_64.tar.gz'; package=name.removesuffix('.tar.gz')
    marker = root/'restarted'
    launcher = b'#!/bin/sh\nprintf restarted > "$GAMEPKG_UPDATE_RECEIPT"\n'
    build = json.dumps({'version':version,'platform':'linux-x86_64','executables_sha256':{name:hashlib.sha256(launcher).hexdigest() for name in ['gamepkg','gamepkg-gui']}}).encode()
    archive=root/'release.tar.gz'
    with tarfile.open(archive,'w:gz') as tar:
        for path, content in [('BUILD.json',build),('bin/gamepkg',launcher),('bin/gamepkg-gui',launcher)]:
            info=tarfile.TarInfo(package+'/'+path); info.size=len(content); info.mode=0o644
            tar.addfile(info,io.BytesIO(content))
    digest=hashlib.sha256(archive.read_bytes()).hexdigest()
    checksum=root/'checksum'; checksum.write_text('0'*64+'  '+name+'\n')
    url=f'https://github.com/centrexity/GamePKG/releases/download/v{version}/'
    metadata=root/'metadata'; metadata.write_text(json.dumps({'tag_name':'v'+version,'draft':False,'prerelease':False,'assets':[
        {'name':name,'size':archive.stat().st_size,'browser_download_url':url+name},
        {'name':name+'.sha256','size':100,'browser_download_url':url+name+'.sha256'}]}))
    curl=transport/'curl'; curl.write_text('''#!/usr/bin/env python3
import os,sys,shutil
from pathlib import Path
args=sys.argv[1:]; url=args[args.index('--url')+1]
source='metadata' if url.endswith('/latest') else 'checksum' if url.endswith('.sha256') else 'release.tar.gz'
shutil.copyfile(Path(os.environ['GAMEPKG_UPDATE_FIXTURES'])/source,args[args.index('--output')+1])
Path(args[args.index('--dump-header')+1]).write_text('HTTP/2 200\\r\\n\\r\\n')
print('200',end='')
'''); curl.chmod(0o755)
    env=dict(os.environ,PATH=str(transport)+os.pathsep+os.environ['PATH'],GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config),GAMEPKG_UPDATE_FIXTURES=str(root),GAMEPKG_UPDATE_RECEIPT=str(marker),LIBGL_ALWAYS_SOFTWARE='1')
    env.pop('WAYLAND_DISPLAY',None); env['DBUS_SESSION_BUS_ADDRESS']='unix:path=/tmp/gamepkg-no-session-bus'
    display=start_display(env,1280,1000)
    app=subprocess.Popen([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',project/'target/release'))/'gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        window,_=game_window(env,'GamePKG')
        click_text(window,env,'Settings',root,max_y=60)
        time.sleep(.5)
        click_text(window,env,'Check for GamePKG updates',root,max_y=650)
        time.sleep(1)
        click_text(window,env,'Download and install GamePKG update',root,max_y=650)
        choose_native_file(env,'Choose where to install the new GamePKG version',destination)
        # Xvfb has no window manager to restore the app's input focus.
        subprocess.run(['xdotool','windowfocus',window,'mousemove','--window',window,'500','100','click','1'],env=env,check=True)
        time.sleep(1)
        click_text(window,env,'integrity check',root,click=False,max_y=1000)
        assert not (destination/package).exists(), 'Damaged download created an installation'
        assert not list(destination.iterdir()), 'Failed update left staging files'
        checksum.write_text(digest+'  '+name+'\n')
        click_text(window,env,'Download and install GamePKG update',root,max_y=650)
        choose_native_file(env,'Choose where to install the new GamePKG version',destination)
        # Xvfb has no window manager to restore the app's input focus.
        subprocess.run(['xdotool','windowfocus',window,'mousemove','--window',window,'500','100','click','1'],env=env,check=True)
        time.sleep(1)
        click_text(window,env,'Restart into updated GamePKG',root,max_y=650)
        app.wait(timeout=15)
        for _ in range(50):
            if marker.exists(): break
            time.sleep(.1)
        assert marker.read_text()=='restarted', 'New version was not launched'
        assert (library/'keep').read_text()=='keep my library'
        assert (destination/package/'bin/gamepkg-gui').is_file()
        print('App updater GUI passed: newer release, native folder picker, corrupt download rejection/cleanup, retry, install, restart, preserved library.')
    finally:
        if app.poll() is None: app.terminate(); app.wait(timeout=10)
        display.terminate(); display.wait(timeout=10)
