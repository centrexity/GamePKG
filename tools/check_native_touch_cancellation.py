"""Run exact native-touch adapter qualification in a private local-only display."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

project=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(project/'tests'))
from gui_test_support import start_display
with tempfile.TemporaryDirectory(prefix='gamepkg-native-touch-') as temporary:
    root=Path(temporary)
    authority=root/'Xauthority';authority.touch(mode=0o600)
    environment=dict(os.environ,XAUTHORITY=str(authority),WINIT_UNIX_BACKEND='x11',
        GAMEPKG_BUNDLED_AUDIO_DECODER=str(project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'),
        GAMEPKG_BUNDLED_AUDIO_CONVERTER=str(project/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'))
    environment.pop('WAYLAND_DISPLAY',None)
    display=start_display(environment,800,600,local_only=True)
    try:
        result=subprocess.run(['cargo','test','--offline','--release','--all-features','--test','native_touch_cancellation','actual_native_touch_cancel_and_focus_loss_never_click_or_stick','--','--ignored','--exact'],cwd=project,env=environment,capture_output=True,text=True,timeout=600)
        if result.returncode:raise RuntimeError(result.stdout+result.stderr)
        assert 'test result: ok. 1 passed; 0 failed; 0 ignored;' in result.stdout,result.stdout
        print(result.stdout,end='')
    finally:
        display.terminate();display.wait(timeout=10)
