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
