"""Start a ready virtual display on an automatically allocated display number."""
import os
import select
import subprocess
import time

def start_display(env, width=1024, height=900, *, local_only=False):
    reader, writer = os.pipe()
    try:
        arguments=['Xvfb', '-displayfd', str(writer), '-screen', '0',
             f'{width}x{height}x24', '-ac', '-noreset']
        if local_only:
            arguments+=['-nolisten','tcp']
        process = subprocess.Popen(
            arguments, pass_fds=(writer,),
            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    finally:
        os.close(writer)
    try:
        ready, _, _ = select.select([reader], [], [], 10)
        if not ready:
            raise RuntimeError('Xvfb did not report a ready display within 10 seconds')
        display = os.read(reader, 32).decode().strip()
        if not display.isdigit():
            raise RuntimeError('Xvfb did not provide a valid display number')
        if process.poll() is not None:
            raise RuntimeError('Xvfb startup failed: ' + process.stderr.read().decode())
        env['DISPLAY'] = ':' + display
        for _ in range(100):
            if process.poll() is not None:
                raise RuntimeError('Xvfb exited before accepting clients')
            connection = subprocess.run(['xdotool', 'getdisplaygeometry'], env=env, capture_output=True, timeout=2)
            if connection.returncode == 0:
                return process
            time.sleep(.1)
        raise RuntimeError('Xvfb did not accept a client connection')
    except BaseException:
        process.terminate()
        process.wait(timeout=10)
        raise
    finally:
        os.close(reader)


def click_text(window, env, text, directory, timeout=15, max_y=None, *, click=True):
    """Locate actual visible control text; never depend on a historical layout."""
    import csv
    import io
    from pathlib import Path
    directory = Path(directory)
    # Bare Xvfb has no window manager to restore focus after a player/picker closes.
    subprocess.run(['xdotool','windowfocus',window],env=env,check=True)
    deadline = time.monotonic() + timeout
    wanted = text.split()
    last = ''
    scroll_attempt = 0
    while True:
        screenshot = directory / 'control-screen.png'
        subprocess.run(['import', '-display', env['DISPLAY'], '-window', window, str(screenshot)], env=env, check=True)
        for name, filters in [('plain', []), ('controls', ['-colorspace', 'Gray', '-negate', '-threshold', '55%']), ('text', ['-colorspace', 'Gray', '-threshold', '65%', '-negate'])]:
            enlarged = directory / f'control-{name}.png'
            crop = ['-crop', f'0x{max_y}+0+0', '+repage'] if max_y is not None else []
            subprocess.run(['convert', str(screenshot), *crop, *filters, '-resize', '300%', str(enlarged)], env=env, check=True)
            tsv = subprocess.check_output(['tesseract', str(enlarged), 'stdout', '--psm', '6' if max_y is not None else '11', 'tsv'], stderr=subprocess.DEVNULL, text=True)
            words = [w for w in csv.DictReader(io.StringIO(tsv), delimiter='\t',quoting=csv.QUOTE_NONE) if w['text'].strip() and (max_y is None or int(w['top'])//3 < max_y)]
            last = ' '.join(w['text'] for w in words)
            # OCR may merge adjacent words in a compact button label.
            for count in range(1, len(wanted)+1):
                for index in range(len(words)-count+1):
                    candidate = words[index:index+count]
                    actual = ''.join(w['text'].strip('[]()|') for w in candidate)
                    if actual.casefold() != ''.join(wanted).casefold():
                        continue
                    if max(int(w['top']) for w in candidate)-min(int(w['top']) for w in candidate)>18:
                        continue
                    first = candidate[0]
                    x = (int(first['left'])+int(first['width'])//2)//3
                    y = (int(first['top'])+int(first['height'])//2)//3
                    if not click:return x,y
                    subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'click','1'],env=env,check=True)
                    return
        if time.monotonic() >= deadline:
            import shutil
            shutil.copy2(screenshot, Path(__file__).resolve().parents[1] / 'target/gui-control-failed.png')
            raise AssertionError(f'Visible control {text!r} was not found: {last}')
        if max_y is None:
            geometry=subprocess.check_output(['xdotool','getwindowgeometry','--shell',window],env=env,text=True)
            size=dict(line.split('=',1) for line in geometry.splitlines() if '=' in line)
            subprocess.run(['xdotool','mousemove','--window',window,str(min(400,int(size['WIDTH'])//2)),str(max(120,int(size['HEIGHT'])-180)),'click','--repeat','3','--delay','35','5' if scroll_attempt%8<4 else '4'],env=env,check=True)
            scroll_attempt+=1
        time.sleep(.1)


def choose_native_file(env, title, path=None, windows=False):
    """Operate the real Linux picker, including cancelling without selecting."""
    import re
    deadline = time.monotonic() + 20
    while True:
        found = subprocess.run(['xdotool','search','--onlyvisible','--name','^('+re.escape(title)+'|Select a [Ff]ile|zenity)$'],env=env,capture_output=True,text=True)
        if found.returncode == 0:
            break
        if time.monotonic() >= deadline:
            from pathlib import Path
            proof=Path(__file__).resolve().parents[1]/'target/native-rom-picker-failed.png'
            subprocess.run(['import','-display',env['DISPLAY'],'-window','root',str(proof)],env=env,check=True)
            names=subprocess.run(['xdotool','search','--onlyvisible','--name','.','getwindowname','%@'],env=env,capture_output=True,text=True)
            raise AssertionError(f'Native picker {title!r} did not open: {names.stdout}')
        time.sleep(.1)
    def area(window):
        geometry = subprocess.run(['xdotool','getwindowgeometry','--shell',window],env=env,capture_output=True,text=True)
        fields = dict(line.split('=',1) for line in geometry.stdout.splitlines() if '=' in line)
        return int(fields.get('WIDTH',0))*int(fields.get('HEIGHT',0))
    # GTK tooltips can share the dialog's WM_NAME. They must never receive the
    # file-selection keystrokes; choose the actual chooser by its visible size.
    dialog = max(found.stdout.splitlines(), key=area)
    subprocess.run(['xdotool','mousemove','--window',dialog,'400','150'],env=env,check=True)
    from pathlib import Path
    proof=Path(__file__).resolve().parents[1]/'target/native-rom-picker.png'
    # GTK4 maps its chooser before the controls are drawn. Sending Ctrl+L at
    # that point can lose the shortcut and type into the wrong widget.
    ready_deadline=time.monotonic()+20
    while True:
        subprocess.run(['import','-display',env['DISPLAY'],'-window',dialog,str(proof)],env=env,check=True)
        text=subprocess.check_output(['tesseract',str(proof),'stdout','--psm','11'],env=env,stderr=subprocess.DEVNULL,text=True).casefold()
        if re.search(r'\bcancel\b',text) and re.search(r'\b(open|select|ok|save)\b',text):
            break
        assert time.monotonic()<ready_deadline, f'Native picker controls were not ready: {text}'
        time.sleep(.1)
    subprocess.run(['xdotool','windowfocus','--sync',dialog],env=env,check=True)
    if path is None:
        subprocess.run(['xdotool','key','--clearmodifiers','Escape'],env=env,check=True)
    else:
        subprocess.run(['xdotool','key','--clearmodifiers','alt+n' if windows else 'ctrl+l'],env=env,check=True)
        time.sleep(.2)
        subprocess.run(['xdotool','key','--clearmodifiers','ctrl+a'],env=env,check=True)
        location=str(path)
        folder=Path(path).is_dir()
        if folder and not windows:
            location=location.rstrip('/')+'/'
        # Send the path as one burst: GTK4's asynchronous completion can
        # otherwise rewrite the entry between synthetic character events.
        subprocess.run(['xdotool','type','--clearmodifiers','--delay','0',location],env=env,check=True)
        time.sleep(.3)
        subprocess.run(['xdotool','key','--clearmodifiers','Return'],env=env,check=True)
    close_deadline=time.monotonic()+15
    while True:
        visible=subprocess.run(['xdotool','search','--onlyvisible','--name','^('+re.escape(title)+'|Select a [Ff]ile|zenity)$'],env=env,capture_output=True,text=True)
        if dialog not in visible.stdout.splitlines():
            break
        if time.monotonic()>=close_deadline:
            subprocess.run(['import','-display',env['DISPLAY'],'-window',dialog,str(proof)],env=env,check=True)
            raise AssertionError(f'Native picker did not finish selecting {path!s}')
        time.sleep(.1)
    return dialog


def game_window(env, game_id, timeout=20):
    """Wait for a separate game window and return its X11 window and process IDs."""
    import re
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        found = subprocess.run(['xdotool', 'search', '--onlyvisible', '--name', '^'+re.escape(game_id)+'$'], env=env, capture_output=True, text=True)
        if found.returncode == 0:
            window = found.stdout.splitlines()[0]
            pid = int(subprocess.check_output(['xdotool', 'getwindowpid', window], env=env, text=True))
            return window, pid
        time.sleep(.1)
    raise AssertionError('Separate game window did not open: '+game_id)


def close_game(window, env, timeout=15):
    """Use the normal window-manager close protocol, then await disappearance."""
    from Xlib import display as xdisplay, protocol
    connection=xdisplay.Display(env['DISPLAY'])
    try:
        target=connection.create_resource_object('window',int(window))
        target.send_event(protocol.event.ClientMessage(window=target,client_type=connection.intern_atom('WM_PROTOCOLS'),data=(32,[connection.intern_atom('WM_DELETE_WINDOW'),0,0,0,0])),event_mask=0)
        connection.flush()
        deadline=time.monotonic()+timeout
        while subprocess.run(['xdotool','getwindowname',window],env=env,capture_output=True).returncode==0:
            assert time.monotonic()<deadline,'Game did not close after WM_DELETE_WINDOW'
            time.sleep(.1)
    finally:
        connection.close()


def wait_game_frame(window, env, directory, color=None, timeout=20):
    """Observe rendered gameplay, optionally a ROM-derived fixture color."""
    from pathlib import Path
    from PIL import Image
    import shutil
    directory=Path(directory)
    capture=directory/'game-frame.png';deadline=time.monotonic()+timeout
    while True:
        result=subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(capture)],env=env,capture_output=True)
        assert result.returncode==0,'Game window disappeared before rendering'
        image=Image.open(capture).convert('RGB')
        center=image.getpixel((image.width//2,image.height//2))
        if (color is not None and all(abs(a-b)<=2 for a,b in zip(center,color))) or (color is None and len(image.getcolors(image.width*image.height))>4):
            return capture
        if time.monotonic()>=deadline:
            proof=Path(__file__).resolve().parents[1]/'target'/('game-frame-'+directory.name+'.png')
            shutil.copy2(capture,proof)
            raise AssertionError(f'Expected rendered game color {color}, got {center}; proof: {proof}')
        time.sleep(.1)


def click_primary_play(window, env, directory, timeout=15):
    """Find the enabled filled Play control, excluding the separate Wasm action."""
    from pathlib import Path
    from PIL import Image
    deadline=time.monotonic()+timeout
    while True:
        path=Path(directory)/'primary-play.png'
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(path)],env=env,check=True)
        image=Image.open(path).convert('RGB');start=None
        for y in range(100,image.height):
            xs=[x for x in range(image.width) if (lambda c:c[0]<100 and c[1]>170 and c[2]>140)(image.getpixel((x,y)))]
            if len(xs)>70:
                if start is None:start=y
                if y-start>=20:
                    subprocess.run(['xdotool','mousemove','--window',window,str(sum(xs)//len(xs)),str(y-10),'click','1'],env=env,check=True)
                    return
            else:start=None
        assert time.monotonic()<deadline,'Enabled primary Play control was not found'
        time.sleep(.1)


def cube2_choose_test_map(window, env):
    """Choose a gameplay fixture through original console input, never launch overrides."""
    subprocess.run(['xdotool','windowfocus',window,'key','slash'],env=env,check=True)
    time.sleep(.3)
    subprocess.run(['xdotool','type','--clearmodifiers','--delay','15','mode 0; map complex; showgui ""'],env=env,check=True)
    subprocess.run(['xdotool','key','Return'],env=env,check=True)
    # Wait for the requested map instead of treating the cached menu/loading
    # image as gameplay. No engine launch flags or hidden startup scripts.
    import tempfile
    from pathlib import Path
    with tempfile.TemporaryDirectory(prefix='cube2-chosen-map-') as directory:
        capture=Path(directory)/'frame.png'
        deadline=time.monotonic()+65
        while True:
            subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(capture)],env=env,check=True)
            from PIL import Image,ImageOps
            image=Image.open(capture).convert('RGB')
            health=image.crop((0,int(image.height*.86),int(image.width*.32),image.height))
            health=ImageOps.grayscale(health).point(lambda p:0 if p>180 else 255).resize((health.width*3,health.height*3))
            hud=Path(directory)/'health.png';health.save(hud)
            # The HUD is one outlined line; block segmentation can split the
            # health digits into its icon or neighboring ammunition count.
            for segmentation in ['6','7']:
                text=subprocess.check_output(['tesseract',str(hud),'stdout','--psm',segmentation,'-c','tessedit_char_whitelist=0123456789'],env=dict(env,OMP_THREAD_LIMIT='1'),stderr=subprocess.DEVNULL,text=True)
                if '100' in ''.join(text.split()):break
            if '100' in ''.join(text.split()):break
            assert time.monotonic()<deadline,'Original console did not enter the requested gameplay map: '+text
            time.sleep(.3)
