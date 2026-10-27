"""Desktop preferences persist only on Save; obsolete crate approvals cannot be enabled."""
from pathlib import Path
import os, subprocess, tempfile, time, tomllib
from gui_test_support import start_display, click_text
os.environ['OMP_THREAD_LIMIT']='1'
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-settings-gui-') as directory:
    root=Path(directory);config=root/'settings.toml'
    config.write_text('automatic_update_checks = false\ncheck_app_updates_on_startup = false\napproved_crates = ["gamepkg-byteorder"]\nsnapshot_load_slots=[10,11,12,13,14,15,16,17,18,19]\nsnapshot_save_slots=[20,11,12,13,14,15,16,17,18,19]\n')
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
            if label in ['Optimize new builds','Limit game render resolution','Snapshot shortcuts']:
                direction='5' if label=='Optimize new builds' else '4'
                subprocess.run(['xdotool','mousemove','--window',window,'400','600','click','--repeat','40','--delay','10',direction],env=env,check=True)
                time.sleep(.3)
            if label == 'Save settings':
                # The help copy also contains this phrase. Select the real
                # action beside Cancel, including after scrolling the form.
                _,save_y=click_text(window,env,'Cancel edits',root,click=False)
                click_text(window,env,label,root,min_y=save_y-20,max_y=save_y+25)
            else:
                click_text(window, env, label, root, max_y=60 if label == 'Settings' else None)
            time.sleep(.3)
        def open_and_toggle():
            click('Settings')
            # This preference can be below the fold. Allow the shared control
            # finder to scroll instead of repeatedly reopening the page.
            click('Optimize new builds')
            time.sleep(.3)
        open_and_toggle(); click('Save settings')
        saved=tomllib.loads(config.read_text())
        assert saved['optimize_builds'] is False, 'Save did not apply optimization preference'
        assert 'approved_crates' not in saved, 'Save retained obsolete crate approvals'
        assert saved['render_limit_enabled'] is False, 'Render resolution limit was enabled by default'
        assert saved['snapshot_load_slots'] == list(range(10,20))
        assert saved['snapshot_save_slots'] == [20]+list(range(11,20))
        click('Optimize new builds'); click('Cancel edits')
        assert tomllib.loads(config.read_text())['optimize_builds'] is False, 'Cancel persisted an unsaved edit'
        open_and_toggle(); click('Save settings')
        assert tomllib.loads(config.read_text())['optimize_builds'] is True, 'Save did not restore optimization'
        click('Limit game render resolution'); click('Cancel edits')
        assert tomllib.loads(config.read_text())['render_limit_enabled'] is False, 'Cancel persisted the render limit'
        click('Settings')
        click('Limit game render resolution'); click('Save settings')
        assert tomllib.loads(config.read_text())['render_limit_enabled'] is True, 'Render limit did not save'
        click('Limit game render resolution'); click('Save settings')
        assert tomllib.loads(config.read_text())['render_limit_enabled'] is False, 'Render limit did not turn off'
        click('Settings');click('Snapshot shortcuts');click('Reset snapshot shortcuts');click('Cancel edits')
        assert tomllib.loads(config.read_text())['snapshot_load_slots'] == list(range(10,20)), 'Cancel persisted shortcut reset'
        click('Settings');click('Reset snapshot shortcuts');click('Save settings')
        saved=tomllib.loads(config.read_text())
        assert saved['snapshot_load_slots'] == list(range(10))
        assert saved['snapshot_save_slots'] == list(range(10))
        assert tomllib.loads(config.read_text())['native_watchdog_seconds'] == 0
        click('Native frame-stall watchdog');click('Cancel edits')
        assert tomllib.loads(config.read_text())['native_watchdog_seconds'] == 0
        click('Settings');click('Native frame-stall watchdog');click('Save settings')
        assert tomllib.loads(config.read_text())['native_watchdog_seconds'] == 30
        click('Native frame-stall watchdog');click('Save settings')
        assert tomllib.loads(config.read_text())['native_watchdog_seconds'] == 0
        output=subprocess.run([str(Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR', project/'target/release'))/'gamepkg'),'--help'],env=env,check=True,capture_output=True,text=True).stdout
        assert 'allowlist' not in output, output
        subprocess.run(['import','-display',env['DISPLAY'],'-window',window,str(project/'target/settings-approval-tested.png')],env=env,check=True)
        print('Settings GUI passed: save, cancel, obsolete approval removal, no crate-management command, watchdog off by default, cancelled enable, saved enable and saved disable.')
    finally:
        app.terminate();app.wait(timeout=10);display.terminate();display.wait(timeout=10)
