"""Desktop preferences persist only on Save; obsolete crate approvals cannot be enabled."""
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
            click_text(window, env, label, root, max_y=60 if label == 'Settings' else None)
            time.sleep(.3)
        def open_and_toggle():
            click('Settings')
            # This preference can be below the fold. Allow the shared control
            # finder to scroll instead of repeatedly reopening the page.
            click_text(window, env, 'Optimize new builds', root)
            time.sleep(.3)
        open_and_toggle(); click('Save settings')
        saved=tomllib.loads(config.read_text())
        assert saved['optimize_builds'] is False, 'Save did not apply optimization preference'
        assert 'approved_crates' not in saved, 'Save retained obsolete crate approvals'
        click('Optimize new builds'); click('Cancel edits')
        assert tomllib.loads(config.read_text())['optimize_builds'] is False, 'Cancel persisted an unsaved edit'
        open_and_toggle(); click('Save settings')
        assert tomllib.loads(config.read_text())['optimize_builds'] is True, 'Save did not restore optimization'
        output=subprocess.run([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', project/'target/release'))/'gamepkg'),'--help'],env=env,check=True,capture_output=True,text=True).stdout
        assert 'allowlist' not in output, output
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(project/'target/settings-approval-tested.png')],env=env,check=True)
        print('Settings GUI passed: save, cancel, obsolete approval removal, and no crate-management command.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
