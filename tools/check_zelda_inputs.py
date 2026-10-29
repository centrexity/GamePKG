"""Maintainer-only fresh native/Wasm Zelda input and gameplay qualification.

Reads the explicitly selected ROM in place. The signed source recipe, temporary
creator key and library are private fixtures; no original media is packaged.
Players do not need this script or Python.
"""
from pathlib import Path
import argparse
import hashlib
import os
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom', type=Path, required=True)
parser.add_argument('--sysroot', type=Path, required=True)
parser.add_argument('--slang-root', type=Path, required=True)
parser.add_argument('--dim-flashes', action='store_true', help='qualify signed dim-flash defaults through gameplay and SRAM')
parser.add_argument('--skip-intro', action='store_true', help='qualify signed early intro skip defaults')
parser.add_argument('--all-enhancements', action='store_true', help='qualify all original gameplay enhancement flags together')
parser.add_argument('--gui-slots', action='store_true', help='also qualify original snapshot keys and restart in private X11/Vulkan windows')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
rom = args.rom.resolve(strict=True)
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-inputs-') as temporary:
    root = Path(temporary)
    settings = root / 'settings.toml'
    settings.write_text('automatic_update_checks=false\n')
    environment = dict(os.environ, GAMEPKG_HOME=str(root / 'library'),
        GAMEPKG_CONFIG=str(settings), GAMEPKG_SLANG_ROOT=str(args.slang_root.resolve()),
        GAMEPKG_ZELDA3_SYSROOT=str(args.sysroot.resolve()),
        GAMEPKG_ZELDA3_TEST_ROOT=str(root / 'library'), GAMEPKG_ZELDA3_TEST_ROM=str(rom),
        GAMEPKG_BUNDLED_AUDIO_DECODER=str(project / 'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'),
        GAMEPKG_BUNDLED_AUDIO_CONVERTER=str(project / 'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'))
    cli = project / 'target/release/gamepkg'
    def run(*command):
        exact_test = command[:2] == ('cargo', 'test')
        result = subprocess.run(list(map(str, command)), cwd=project, env=environment,
            capture_output=exact_test, text=True, timeout=600)
        if exact_test:
            if result.returncode:
                raise RuntimeError(result.stdout[-6000:] + result.stderr[-6000:])
            if 'test result: ok. 1 passed; 0 failed; 0 ignored;' not in result.stdout:
                raise RuntimeError('The requested exact Zelda regression did not execute once: ' + result.stdout[-6000:])
        result.check_returncode()
    run(cli, 'keygen', '-o', root / 'creator.key')
    recipe=project/'ports/zelda3'
    if args.dim_flashes or args.skip_intro or args.all_enhancements:
        shutil.copytree(recipe,root/'recipe');recipe=root/'recipe'
        with (recipe/'manifest.toml').open('a') as manifest:
            manifest.write('\n[zelda_video]\ndim_flashes='+str(args.dim_flashes).lower()+'\nskip_intro_on_keypress='+str(args.skip_intro).lower()+'\n')
            if args.all_enhancements:
                manifest.write('[zelda_video.enhancements]\n')
                for name in ['item_switch_lr', 'item_switch_lr_limit', 'turn_while_dashing', 'mirror_to_darkworld', 'collect_items_with_sword', 'break_pots_with_sword', 'disable_low_health_beep', 'show_max_items_in_yellow', 'more_active_bombs', 'carry_more_rupees', 'misc_bug_fixes', 'game_changing_bug_fixes', 'cancel_bird_travel']:manifest.write(name+'=true\n')
    run(cli, 'pack', recipe, '-o', root / 'zelda3.gamepkg', '--key', root / 'creator.key')
    run(cli, 'install', root / 'zelda3.gamepkg', '--trust-creator')
    for target in ['native', 'wasm']:
        run(cli, 'build', 'com.gamepkg.zelda3', '--target', target)
        environment['GAMEPKG_ZELDA3_TEST_TARGET'] = target
        run('cargo', 'test', '--locked', '--release', '--all-features', '--lib', 'zelda_snapshots::tests::actual_corrupt_snapshots_preserve_running_game_and_wrapper_state', '--', '--ignored', '--exact')
        for test in ['actual_zelda3_host_snapshots_restore_without_restarting_and_replay_exactly', 'actual_zelda3_opposing_directions_follow_original_filter', 'actual_zelda3_frames', 'actual_zelda3_stereo_title_preserves_channel_differences', 'actual_zelda3_early_intro_skip_preserves_original_default', 'actual_zelda3_all_logical_buttons_follow_original_joypad_mapping']:
            run('cargo', 'test', '--locked', '--release', '--all-features', '--test', 'zelda3_play', test, '--', '--ignored', '--exact')
        if target=='wasm':
            run('cargo','test','--locked','--release','--all-features','--test','zelda3_play','actual_zelda3_stale_renderer_is_rejected_and_integrity_fix_rebuilds','--','--ignored','--exact')
        if args.gui_slots:
            run('python3', project / 'tools/check_zelda_slots_gui.py', '--root', root / 'library', '--rom', rom, '--target', target)
            run('python3', project / 'tools/check_zelda_slots_gui.py', '--root', root / 'library', '--rom', rom, '--target', target, '--remap-slots')
        run(cli, 'run', 'com.gamepkg.zelda3', '--target', target, '--rom', rom, '--frames', '200', '--save-slot', '20')
        run(cli, 'run', 'com.gamepkg.zelda3', '--target', target, '--rom', rom, '--frames', '1', '--load-slot', '20', '--save-slot', '19')
        for number, counter in [(20, 200), (19, 201)]:
            slots=list((root/'library/states').glob(f'*-{number:02}.gpstate'))
            assert len(slots)==1
            saved=slots[0].read_bytes()
            assert len(saved)==96+275497 and saved[:8]==b'GPZHSLOT'
            assert saved[64:96]==hashlib.sha256(saved[:64]+saved[96:]).digest()
            assert int.from_bytes(saved[56:64],'little')==counter
        print(target + ': opposing directions, original gameplay, stereo and SRAM reload passed.', flush=True)
