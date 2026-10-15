"""Desktop approval edits persist only on Save and are shared with the CLI."""
from pathlib import Path
import os, subprocess, tempfile, time, tomllib
from gui_test_support import start_display, click_text
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-settings-gui-') as directory:
    root=Path(directory);config=root/'settings.toml'
    config.write_text('automatic_update_checks = false\napproved_crates = ["gamepkg-byteorder"]\n')
    env=dict(os.environ,GAMEPKG_HOME=str(root/'library'),GAMEPKG_CONFIG=str(config),LIBGL_ALWAYS_SOFTWARE='1');env.pop('WAYLAND_DISPLAY',None)
    display=start_display(env,1200,1200)
    app=subprocess.Popen([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', project/'target/release'))/'gamepkg-gui')],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    try:
        for _ in range(100):
            found=subprocess.run(['xdotool','search','--onlyvisible','--name','^GamePKG$'],env=env,capture_output=True,text=True)
            if found.returncode==0:break
            if app.poll() is not None:raise RuntimeError(app.stderr.read().decode())
            time.sleep(.1)
        else:raise AssertionError('GUI did not open')
        window=found.stdout.splitlines()[0]
        subprocess.run(['xdotool','windowsize',window,'1100','1100'],env=env,check=True);time.sleep(.3)
        def click(label):
            click_text(window, env, label, root, max_y=40 if label == 'Settings' else None)
            time.sleep(.3)
        def open_and_toggle():
            for attempt in range(5):
                click('Settings')
                try:
                    click_text(window, env, 'Allow bundled gamepkg-byteorder', root, timeout=2)
                    time.sleep(.3)
                    return
                except AssertionError:
                    if attempt == 4: raise
        open_and_toggle(); click('Save settings')
        assert tomllib.loads(config.read_text())['approved_crates']==[], 'Save did not revoke the library approval'
        click('Allow bundled gamepkg-byteorder'); click('Cancel edits')
        assert tomllib.loads(config.read_text())['approved_crates']==[], 'Cancel persisted an unsaved approval'
        open_and_toggle(); click('Save settings')
        assert tomllib.loads(config.read_text())['approved_crates']==['gamepkg-byteorder'], 'Save did not restore approval'
        output=subprocess.run([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', project/'target/release'))/'gamepkg'),'allowlist','list'],env=env,check=True,capture_output=True,text=True).stdout
        assert '— approved —' in output, output
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(project/'target/settings-approval-tested.png')],env=env,check=True)
        print('Settings GUI passed: revoke, save, cancel unsaved restoration, restore, and CLI policy sharing.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
