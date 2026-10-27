"""Real game-page display controls, saved values and application restart."""
from pathlib import Path
import json,os,re,shutil,subprocess,tempfile,time
from gui_test_support import start_display,game_window,click_text
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-display-gui-') as temporary:
    root=Path(temporary);recipe=root/'recipe';library=root/'library';settings=root/'settings.toml'
    shutil.copytree(project/'ports/zelda3',recipe)
    path=recipe/'manifest.toml';path.write_text(path.read_text().replace('title = "Zelda3"','title = "Display Test"',1)+'\n[zelda_video]\nextra_width=32\nextend_height=false\n')
    settings.write_text('automatic_update_checks=false\ncheck_app_updates_on_startup=false\n')
    environment=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(settings),LIBGL_ALWAYS_SOFTWARE='1',LP_NUM_THREADS='2',OMP_THREAD_LIMIT='1',DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    environment.pop('WAYLAND_DISPLAY',None)
    key=root/'key';key.write_text('64'*32)
    for command in [('pack',recipe,'-o',root/'source.gamepkg','--key',key),('install',root/'source.gamepkg','--trust-creator')]:
        subprocess.run([str(project/'target/release/gamepkg'),*map(str,command)],env=environment,check=True,capture_output=True,text=True,timeout=30)
    server=start_display(environment,1500,2200);app=None
    try:
        def launch():
            global app
            app=subprocess.Popen([str(project/'target/release/gamepkg-gui')],env=environment,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            window,_=game_window(environment,'GamePKG')
            subprocess.run(['xdotool','windowsize',window,'1400','2100','windowfocus',window],env=environment,check=True)
            click_text(window,environment,'Display Test',root,max_y=800);time.sleep(.3)
            return window
        def stop():
            app.terminate();app.wait(timeout=15)
        def values():
            file=library/'video-selections.json'
            return json.loads(file.read_text()) if file.exists() else {}
        def wait_value(expected):
            deadline=time.monotonic()+15
            while True:
                entries=values()
                if list(entries.values())==expected:
                    # The worker commits the file before the UI consumes its
                    # completion. Wait for the visible, enabled form as well.
                    click_text(window,environment,'Display settings saved',root,click=False)
                    return
                if time.monotonic()>=deadline:
                    subprocess.run(['import','-display',environment['DISPLAY'],'-window',window,'/tmp/gamepkg-display-save-failure.png'],env=environment,check=True)
                    raise AssertionError((entries,expected))
                time.sleep(.1)
        def wait_visible(window,pattern):
            deadline=time.monotonic()+15
            while True:
                shot=root/'visible-state.png'
                subprocess.run(['import','-display',environment['DISPLAY'],'-window',window,str(shot)],env=environment,check=True)
                text=subprocess.check_output(['tesseract',str(shot),'stdout','--psm','11'],stderr=subprocess.DEVNULL,text=True)
                if re.search(pattern,text,re.I):return
                if time.monotonic()>=deadline:
                    shutil.copy2(shot,project/'target/zelda-display-failure.png')
                    diagnostic={'expected_visible':pattern,'observed_text':text,'saved_selections':values()}
                    (project/'target/zelda-display-failure.json').write_text(json.dumps(diagnostic,indent=2))
                    raise AssertionError(diagnostic)
                time.sleep(.1)
        def select_view(window,current,choice):
            click_text(window,environment,current,root)
            # Observe the popup before searching for its option. Otherwise a
            # failed opening can scroll the underlying form instead.
            wait_visible(window,r'Widest\s+view')
            click_text(window,environment,choice,root)
        def image_size(window,width,height):
            click_text(window,environment,'Game image:',root,click=False)
            subprocess.run(['xdotool','mousemove','--window',window,'1300','60'],env=environment,check=True);time.sleep(.2)
            shot=root/'screen.png';subprocess.run(['import','-display',environment['DISPLAY'],'-window',window,str(shot)],env=environment,check=True)
            text=subprocess.check_output(['tesseract',str(shot),'stdout','--psm','11'],stderr=subprocess.DEVNULL,text=True)
            assert re.search(rf'\b{width}\s*[x×]\s*{height}\b',text),text
        window=launch();click_text(window,environment,'Package default',root,click=False);image_size(window,320,224)
        click_text(window,environment,'Allow extra sprites',root);click_text(window,environment,'Extended vertical view',root,click=False);click_text(window,environment,'Dim flashes',root);click_text(window,environment,'Skip intro earlier on button press',root);image_size(window,320,224)
        click_text(window,environment,'Gameplay enhancements',root)
        click_text(window,environment,'Press X while choosing',root,click=False)
        click_text(window,environment,'Allow canceling bird travel',root)
        click_text(window,environment,'Cycle items with shoulder buttons',root)
        click_text(window,environment,'Limit shoulder cycling to the first four slots',root)
        click_text(window,environment,'Disable the low-health beep',root)
        click_text(window,environment,'Save display settings',root)
        wait_value([{'extra_width':32,'extend_height':False,'no_sprite_limits':True,'dim_flashes':True,'skip_intro_on_keypress':True,'enhancements':{'item_switch_lr':True,'item_switch_lr_limit':True,'disable_low_health_beep':True,'cancel_bird_travel':True}}])
        stop();window=launch();click_text(window,environment,'Custom',root,click=False);image_size(window,320,224)
        select_view(window,'Custom','Widescreen')
        click_text(window,environment,'Extended vertical view',root);image_size(window,426,240)
        click_text(window,environment,'Save display settings',root)
        expected=[{'extra_width':85,'extend_height':True,'no_sprite_limits':True,'dim_flashes':True,'skip_intro_on_keypress':True,'enhancements':{'item_switch_lr':True,'item_switch_lr_limit':True,'disable_low_health_beep':True,'cancel_bird_travel':True}}];wait_value(expected)
        # A change without Save must not leak into the stored launch preference.
        select_view(window,'Widescreen','Original width')
        assert list(values().values())==expected
        stop();window=launch();click_text(window,environment,'Widescreen',root,click=False);image_size(window,426,240)
        select_view(window,'Widescreen','Package default')
        image_size(window,320,224);click_text(window,environment,'Save display settings',root);wait_value([])
        stop();window=launch();click_text(window,environment,'Package default',root,click=False);image_size(window,320,224)
        print('Real GUI: independent presentation and gameplay toggles, preserved custom width, widescreen/extended selection, explicit save, unsaved change, restart and package-default reset passed.',flush=True)
    finally:
        if app is not None and app.poll() is None:stop()
        server.terminate();server.wait(timeout=10)
