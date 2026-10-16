"""Start a ready virtual display on an automatically allocated display number."""
import os
import select
import subprocess
import time

def start_display(env, width=1024, height=900):
    reader, writer = os.pipe()
    try:
        process = subprocess.Popen(
            ['Xvfb', '-displayfd', str(writer), '-screen', '0',
             f'{width}x{height}x24', '-ac', '-noreset'], pass_fds=(writer,),
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


def click_text(window, env, text, directory, timeout=15, max_y=None):
    """Locate actual visible control text; never depend on a historical layout."""
    import csv
    import io
    from pathlib import Path
    directory = Path(directory)
    deadline = time.monotonic() + timeout
    wanted = text.split()
    last = ''
    scroll_attempt = 0
    while True:
        screenshot = directory / 'control-screen.png'
        subprocess.run(['import', '-display', env['DISPLAY'], '-window', window, str(screenshot)], env=env, check=True)
        for name, filters in [('plain', []), ('controls', ['-colorspace', 'Gray', '-negate', '-threshold', '55%'])]:
            enlarged = directory / f'control-{name}.png'
            subprocess.run(['convert', str(screenshot), *filters, '-resize', '300%', str(enlarged)], env=env, check=True)
            tsv = subprocess.check_output(['tesseract', str(enlarged), 'stdout', '--psm', '11', 'tsv'], stderr=subprocess.DEVNULL, text=True)
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
                    subprocess.run(['xdotool','mousemove','--window',window,str(x),str(y),'click','1'],env=env,check=True)
                    return
        if time.monotonic() >= deadline:
            raise AssertionError(f'Visible control {text!r} was not found: {last}')
        if max_y is None:
            geometry=subprocess.check_output(['xdotool','getwindowgeometry','--shell',window],env=env,text=True)
            size=dict(line.split('=',1) for line in geometry.splitlines() if '=' in line)
            subprocess.run(['xdotool','mousemove','--window',window,str(int(size['WIDTH'])-28),str(max(120,int(size['HEIGHT'])-180)),'click','--repeat','3','--delay','35','5' if scroll_attempt%8<4 else '4'],env=env,check=True)
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
    dialog = found.stdout.splitlines()[-1]
    from pathlib import Path
    proof=Path(__file__).resolve().parents[1]/'target/native-rom-picker.png'
    subprocess.run(['import','-display',env['DISPLAY'],'-window',dialog,str(proof)],env=env,check=True)
    subprocess.run(['xdotool','windowfocus','--sync',dialog],env=env,check=True)
    if path is None:
        subprocess.run(['xdotool','key','--clearmodifiers','Escape'],env=env,check=True)
    else:
        subprocess.run(['xdotool','key','--clearmodifiers','alt+n' if windows else 'ctrl+l'],env=env,check=True)
        time.sleep(.2)
        subprocess.run(['xdotool','type','--clearmodifiers',str(path)],env=env,check=True)
        subprocess.run(['xdotool','key','--clearmodifiers','Return'],env=env,check=True)
    time.sleep(.5)


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
