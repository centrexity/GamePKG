"""Private signed CLI preferences, unsupported profiles and corrupt metadata."""
from pathlib import Path
import json,os,subprocess,tempfile
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-display-cli-') as temporary:
    root=Path(temporary);library=root/'library';settings=root/'settings.toml'
    settings.write_text('automatic_update_checks=false\n')
    environment=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(settings))
    key=root/'key';key.write_text('65'*32)
    def run(*args,ok=True):
        result=subprocess.run([str(project/'target/release/gamepkg'),*map(str,args)],env=environment,capture_output=True,text=True,timeout=30)
        assert (result.returncode==0)==ok,(args,result.stdout,result.stderr)
        return result
    for game in ['zelda3','doom']:
        package=root/f'{game}.gamepkg'
        run('pack',project/'ports'/game,'-o',package,'--key',key)
        run('install',package,'--trust-creator')
    file=library/'video-selections.json'
    for name in ['item_switch_lr', 'item_switch_lr_limit', 'turn_while_dashing', 'mirror_to_darkworld', 'collect_items_with_sword', 'break_pots_with_sword', 'disable_low_health_beep', 'show_max_items_in_yellow', 'more_active_bombs', 'carry_more_rupees', 'misc_bug_fixes', 'game_changing_bug_fixes', 'cancel_bird_travel']:
        run('configure-display','com.gamepkg.zelda3','--preset','original','--enhancement',name)
        assert list(json.loads(file.read_bytes()).values())==[{'extra_width':0,'extend_height':False,'enhancements':{name:True}}]
    run('configure-display','com.gamepkg.zelda3','--preset','wide','--extended-height','--extra-sprites','--dim-flashes','--skip-intro','--enhancement','item_switch_lr','--enhancement','disable_low_health_beep')
    file=library/'video-selections.json';before=file.read_bytes()
    assert list(json.loads(before).values())==[{'extra_width':85,'extend_height':True,'no_sprite_limits':True,'dim_flashes':True,'skip_intro_on_keypress':True,'enhancements':{'item_switch_lr':True,'disable_low_health_beep':True}}]
    for args in [
        ['com.gamepkg.zelda3','--preset','default','--extended-height'],
        ['com.gamepkg.zelda3','--preset','default','--extra-sprites'],
        ['com.gamepkg.zelda3','--preset','default','--dim-flashes'],
        ['com.gamepkg.zelda3','--preset','default','--skip-intro'],
        ['com.gamepkg.zelda3','--preset','default','--enhancement','item_switch_lr'],
        ['com.gamepkg.zelda3','--preset','original','--enhancement','invented'],
        ['com.gamepkg.zelda3','--preset','unknown'],
        ['com.gamepkg.doom','--preset','wide'],
        ['../../outside','--preset','wide']]:
        run('configure-display',*args,ok=False)
        assert file.read_bytes()==before
    run('integrity-check','com.gamepkg.zelda3')
    damaged=b'{"invalid": {"extra_width": 999}}';file.write_bytes(damaged)
    result=run('integrity-check','com.gamepkg.zelda3',ok=False)
    assert 'invalid saved display settings' in result.stderr,result.stderr
    run('configure-display','com.gamepkg.zelda3','--preset','default',ok=False)
    assert file.read_bytes()==damaged
    file.write_bytes(before)
    run('configure-display','com.gamepkg.zelda3','--preset','default')
    assert json.loads(file.read_bytes())=={}
    run('integrity-check','com.gamepkg.zelda3')
print('Signed display CLI, invalid requests, corrupt metadata preservation, integrity checks and default reset passed.')
