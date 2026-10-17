"""Real X11 capture/hidden cursor regression using the cached original engine.
Run tests/cube2_package.py first. Uses only that maintainer fixture's library.
"""
from gui_test_support import cube2_choose_test_map
from pathlib import Path
import ctypes, os, subprocess, tempfile, time
from Xlib import X, display
from gui_test_support import start_display, game_window, close_game, wait_game_frame

PROJECT = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-mouse-') as temporary:
    root = Path(temporary)
    config = root / 'settings.toml'
    config.write_text('memory_ceiling_mib=2048\ntick_deadline_ms=500\ncheck_app_updates_on_startup=false\nautomatic_update_checks=false\nmute_audio=true\n')
    env = dict(os.environ, GAMEPKG_HOME=str(PROJECT/'target/cube2-package-test/library'),
               GAMEPKG_CONFIG=str(config), LIBGL_ALWAYS_SOFTWARE='1', LP_NUM_THREADS='2',
               GAMEPKG_RENDERER='vulkan', DBUS_SESSION_BUS_ADDRESS='unix:path=/tmp/gamepkg-no-session-bus')
    env.pop('WAYLAND_DISPLAY', None)
    server = start_display(env, 1400, 1000)
    proof = PROJECT/'target/cube2-mouse-gui'
    proof.mkdir(exist_ok=True)
    app = None
    connection = None
    def command(*args):
        return subprocess.run(list(map(str,args)), env=env, check=True, capture_output=True, timeout=15)
    try:
        app = subprocess.Popen([str(PROJECT/'target/release/gamepkg-gui'),'--play','com.gamepkg.cube2'],
                               env=env, stdout=subprocess.DEVNULL, stderr=(proof/'player.log').open('wb'))
        window, _ = game_window(env, 'com.gamepkg.cube2', timeout=30)
        connection = display.Display(env['DISPLAY'])
        screen = connection.screen()
        other = screen.root.create_window(1200, 30, 100, 100, 0, screen.root_depth, X.InputOutput, X.CopyFromParent,
                                          background_pixel=screen.white_pixel)
        other.map(); connection.sync()
        def captured():
            status = screen.root.grab_pointer(False, 0, X.GrabModeAsync, X.GrabModeAsync, X.NONE, X.NONE, X.CurrentTime)
            if status == X.GrabSuccess:
                connection.ungrab_pointer(X.CurrentTime); connection.sync()
                return False
            assert status == X.AlreadyGrabbed, status
            return True
        def wait_capture(expected):
            deadline = time.monotonic()+5
            while captured() != expected:
                assert app.poll() is None, (proof/'player.log').read_text()
                assert time.monotonic() < deadline, f'Expected captured={expected}'
                time.sleep(.1)
        x11 = ctypes.CDLL('libX11.so.6')
        fixes = ctypes.CDLL('libXfixes.so.3')
        class Cursor(ctypes.Structure):
            _fields_=[('x',ctypes.c_short),('y',ctypes.c_short),('width',ctypes.c_ushort),('height',ctypes.c_ushort),
                      ('xhot',ctypes.c_ushort),('yhot',ctypes.c_ushort),('serial',ctypes.c_ulong),('pixels',ctypes.POINTER(ctypes.c_ulong))]
        x11.XOpenDisplay.argtypes=[ctypes.c_char_p]; x11.XOpenDisplay.restype=ctypes.c_void_p
        native=x11.XOpenDisplay(env['DISPLAY'].encode())
        fixes.XFixesGetCursorImage.argtypes=[ctypes.c_void_p]; fixes.XFixesGetCursorImage.restype=ctypes.POINTER(Cursor)
        x11.XFree.argtypes=[ctypes.c_void_p]
        def visible():
            image=fixes.XFixesGetCursorImage(native)
            try:
                return any((image.contents.pixels[i] >> 24)&255 for i in range(image.contents.width*image.contents.height))
            finally: x11.XFree(image)
        command('xdotool','windowfocus',window)
        wait_game_frame(window, env, proof, timeout=30)
        cube2_choose_test_map(window,env)
        wait_capture(True); time.sleep(.3)
        assert not visible(), 'Captured system cursor is visible'
        from PIL import Image, ImageChops
        def scene(name):
            path=proof/name
            command('import','-display',env['DISPLAY'],'-window',window,path)
            return Image.open(path).convert('RGB')
        before=scene('before-look.png')
        for _ in range(5):
            command('xdotool','mousemove_relative','--','80','0')
            time.sleep(.08)
        time.sleep(.3)
        after=scene('after-look.png')
        assert ImageChops.difference(before,after).convert('L').point(lambda p: 255 if p>20 else 0).histogram()[255]>10000, 'Locked raw mouse motion did not turn the view'
        command('xdotool','key','Escape');time.sleep(.4)
        wait_capture(True)
        assert not visible(), 'Escape/menu revealed the system cursor'
        command('xdotool','keydown','Control_L','keydown','Alt_L');time.sleep(.3)
        command('xdotool','keyup','Alt_L','keyup','Control_L');time.sleep(.2)
        wait_capture(False)
        assert visible(), 'Explicit release left cursor hidden'
        # The temporary pointer probe can change focus notifications on bare Xvfb.
        command('xdotool','windowfocus',window)
        time.sleep(.2)
        command('xdotool','mousemove','--window',window,'400','300')
        time.sleep(.2)
        command('xdotool','click','1')
        wait_capture(True);time.sleep(.2)
        assert not visible(), 'Click did not hide cursor again'
        command('xdotool','windowfocus',other.id)
        wait_capture(False);time.sleep(.2)
        assert visible(), 'Unfocused game left cursor hidden'
        command('xdotool','windowfocus',window)
        wait_capture(True);time.sleep(.2)
        assert not visible(), 'Refocused game did not hide cursor'
        close_game(window,env)
        assert app.wait(timeout=15)==0, (proof/'player.log').read_text()
        wait_capture(False)
        x11.XCloseDisplay.argtypes=[ctypes.c_void_p];x11.XCloseDisplay(native)
        print('Cube 2 mouse passed: native pointer lock/hide, Escape menu, Ctrl+Alt release, click recapture, focus loss/restore and close.')
    finally:
        if app and app.poll() is None:
            command('import','-display',env['DISPLAY'],'-window','root',proof/'last-root.png')
        if app and app.poll() is None: app.terminate();app.wait(timeout=10)
        if connection: connection.close()
        server.terminate();server.wait(timeout=10)
