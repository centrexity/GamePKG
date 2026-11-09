#!/usr/bin/env python3
"""Maintainer check of pinned graphics defaults and real settings precedence."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def run(args, **kwargs):
    result = subprocess.run(args, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', required=True, type=Path)
    parser.add_argument('--cc', default='clang-18')
    parser.add_argument('--m32', action='store_true')
    parser.add_argument('--sanitize', action='store_true')
    parser.add_argument('--system-include', action='append', type=Path, default=[],
                        help='Additional fallback system headers for maintainer multilib setups')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    bridge = project / 'engines/takedawn/media-bridge'
    src = args.toolkit.resolve() / 'src'
    with tempfile.TemporaryDirectory(prefix='gamepkg-graphics-defaults-') as temporary:
        work = Path(temporary)
        original = src / 'overlay/xbox_settings.c'
        settings = work / 'xbox_settings.c'

        def stage(source, target):
            return subprocess.run(['cmake', '-DINPUT=' + str(source),
                                   '-DOUTPUT=' + str(target), '-DKIND=settings',
                                   '-P', str(bridge / 'adapt_presentation.cmake')],
                                  capture_output=True, text=True)

        before = original.read_bytes()
        result = stage(original, settings)
        assert result.returncode == 0, result.stderr
        assert original.read_bytes() == before
        assert stage(original, original).returncode != 0
        assert original.read_bytes() == before
        alias = work / 'alias.c'
        alias.symlink_to(original)
        assert stage(original, alias).returncode != 0
        corrupted = work / 'changed.c'
        corrupted.write_bytes(before + b'\n/* unexpected source */\n')
        rejected = work / 'rejected.c'
        assert stage(corrupted, rejected).returncode != 0
        assert not rejected.exists()

        # Exercise the actual upstream reader, writer, live application and
        # precedence suite. Only expectations for our selected defaults change.
        test = (args.toolkit / 'tests/overlay_settings/test_main.c').read_text()
        test = test.replace('XS_GRAPHICS_LOD_BIAS) == 0 &&',
                            'XS_GRAPHICS_LOD_BIAS) == 1 &&')
        test = test.replace('CHECK(xbox_SettingValue(XS_GRAPHICS_UI_SCALE) == 0);',
                            'CHECK(xbox_SettingValue(XS_GRAPHICS_UI_SCALE) == 1);')
        test = test.replace('xbox_SettingReset(XS_GRAPHICS_UI_SCALE);\n    CHECK(st_ui_scale == 0);',
                            'xbox_SettingReset(XS_GRAPHICS_UI_SCALE);\n    CHECK(st_ui_scale == 2);')
        test = test.replace('xbox_SettingSet(XS_GRAPHICS_RENDER_SCALE, 2);\n    CHECK(xbox_SettingsNeedRestart());\n    xbox_SettingSet(XS_GRAPHICS_RENDER_SCALE, 1);',
                            'xbox_SettingSet(XS_GRAPHICS_RENDER_SCALE, 1);\n    CHECK(xbox_SettingsNeedRestart());\n    xbox_SettingSet(XS_GRAPHICS_RENDER_SCALE, 2);')
        test = test.replace('xbox_SettingSet(XS_GRAPHICS_UI_SCALE, 2);', 'xbox_SettingSet(XS_GRAPHICS_UI_SCALE, 3);')
        test = test.replace('st_ui_scale == 4 && xbox_SettingValue(XS_GRAPHICS_UI_SCALE) == 2',
                            'st_ui_scale == 4 && xbox_SettingValue(XS_GRAPHICS_UI_SCALE) == 3')
        test = test.replace('CHECK(xbox_SettingValue(XS_INPUT_GLYPHS) == 0);',
                            'CHECK(xbox_SettingValue(XS_INPUT_GLYPHS) == 1);')
        test = test.replace('CHECK(st_glyphs == 0);', 'CHECK(st_glyphs == 1);')
        checks = '\n'.join(
            f'    CHECK(xbox_SettingGet({key})->builtin == {value});'
            for key, value in [('XS_INPUT_GLYPHS', '1'), ('XS_GRAPHICS_RENDER_SCALE', '2'),
                               ('XS_DISPLAY_ASPECT', '1'), ('XS_DISPLAY_HUD_FIX', '1'),
                               ('XS_GRAPHICS_AA', '2'), ('XS_GRAPHICS_SHARPEN', '0.10'),
                               ('XS_GRAPHICS_LOD_BIAS', '1'), ('XS_GRAPHICS_UI_SCALE', '1')])
        test = test.replace('    xbox_SettingsInit("Test");',
                            '    xbox_SettingsInit("Test");\n' + checks)
        # Read host launch controls with the real settings parser, including a
        # conflicting saved settings file. No graphics device is needed here.
        launch_check = r'''int main(int argc, char **argv)
{
    if (getenv("GAMEPKG_CHECK_LAUNCH")) {
        static const int ids[] = { XS_DISPLAY_FULLSCREEN, XS_DISPLAY_ASPECT,
            XS_GRAPHICS_RENDER_SCALE, XS_GRAPHICS_AA, XS_GRAPHICS_LOD_BIAS,
            XS_GRAPHICS_SHARPEN, XS_DISPLAY_HUD_FIX, XS_GRAPHICS_UI_SCALE };
        char expected[256];
        snprintf(expected, sizeof expected, "%s", getenv("GAMEPKG_EXPECT_LAUNCH"));
        for (int i = 1; i < argc; i++) {
            int handled = gamepkg_launch_option(argc, argv, &i);
            if (handled <= 0) return 2;
        }
        recomp_env_reload();
        xbox_SettingsInit("Test");
        xbox_SettingsFinish();
        char *part = strtok(expected, ",");
        for (unsigned i = 0; i < sizeof ids / sizeof ids[0]; i++) {
            CHECK(part != NULL);
            if (!part) return 1;
            double diff = xbox_SettingValue(ids[i]) - atof(part);
            CHECK(diff > -0.000001 && diff < 0.000001);
            CHECK(xbox_SettingGet(ids[i])->locked);
            part = strtok(NULL, ",");
        }
        CHECK(part == NULL);
        xbox_SettingsFlush();
        extern void gamepkg_ToggleFullscreen(void);
        double initial = xbox_SettingValue(XS_DISPLAY_FULLSCREEN);
        gamepkg_ToggleFullscreen();
        CHECK(xbox_SettingValue(XS_DISPLAY_FULLSCREEN) == !initial);
        CHECK(st_opts.fullscreen == !initial);
        CHECK(xbox_SettingGet(XS_DISPLAY_FULLSCREEN)->locked);
        gamepkg_ToggleFullscreen();
        CHECK(xbox_SettingValue(XS_DISPLAY_FULLSCREEN) == initial);
        CHECK(st_opts.fullscreen == initial);
        CHECK(xbox_SettingGet(XS_DISPLAY_FULLSCREEN)->locked);
        extern void gamepkg_SetFullscreen(int fullscreen);
        for (int fullscreen = 0; fullscreen != 2; fullscreen++) {
            for (int repeat = 0; repeat != 2; repeat++) {
                gamepkg_SetFullscreen(fullscreen);
                CHECK(xbox_SettingValue(XS_DISPLAY_FULLSCREEN) == fullscreen);
                CHECK(st_opts.fullscreen == fullscreen);
                CHECK(xbox_SettingGet(XS_DISPLAY_FULLSCREEN)->locked);
            }
        }
        (fprintf)(stderr, "launch controls: %d checks, %d failed\n", s_checks, s_fail);
        return s_fail ? 1 : 0;
    }
'''
        assert test.count('int main(void)\n{') == 1
        test = test.replace('int main(void)\n{', launch_check, 1)
        fixture = work / 'test_main.c'
        fixture.write_text('#include \"launch_args.h\"\n' + test)
        command = [args.cc, '-std=c11', '-D_POSIX_C_SOURCE=200809L',
                   '-DRECOMP_ENV_HAVE_ENHANCE_KEYS', '-I', str(bridge), '-O2', '-Wall', '-Wextra']
        if args.m32:
            command.append('-m32')
        if args.sanitize:
            command += ['-fsanitize=address,undefined', '-fno-omit-frame-pointer']
        for directory in args.system_include:
            command += ['-idirafter', str(directory.resolve())]
        for directory in ['overlay', 'enhance', 'platform', 'input', 'kernel', 'apu', 'd3d', '']:
            command += ['-I', str(src / directory)]
        command += [str(fixture), str(settings)]
        command += [str(src / name) for name in ['enhance/recomp_cfg.c',
                    'platform/recomp_env.c', 'platform/recomp_save_dir.c', 'platform/xr_log.c']]
        executable = work / 'settings-check'
        run(command + ['-lm', '-o', str(executable)])
        result = run([str(executable)], cwd=work,
                     env=dict(os.environ, ASAN_OPTIONS='detect_leaks=0'))
        saved = work / 'launch-settings.toml'
        saved_bytes = (b'[display]\nfullscreen = false\naspect = "4:3"\nhud_fix = false\n'
                       b'[graphics]\nrender_scale = 1\naa = "off"\nlod_bias = "off"\n'
                       b'sharpen = 0.0\nui_scale = "off"\n')
        keys = ['RECOMP_PRESENT_FULLSCREEN', 'RECOMP_DISPLAY_WIDESCREEN',
                'RECOMP_RENDER_SCALE', 'RECOMP_GFX_AA', 'RECOMP_GFX_LOD_BIAS',
                'RECOMP_GFX_SHARPEN', 'RECOMP_GFX_HUD_FIX', 'RECOMP_GFX_UI_SCALE']
        matrix = [(['windowed', '16:9', '2', 'smaa', 'auto', '10', 'true', '2'], '0,1,2,2,1,0.10,1,1'),
                  (['fullscreen', '4:3', '1', 'off', 'off', '0', 'false', '1'], '1,0,1,0,0,0,0,0'),
                  (['fullscreen', '16:9', '4', 'fxaa', '-1.5', '100', 'true', '4'], '1,1,4,1,4,1,1,3'),
                  (['windowed', '16:9', '3', 'smaa', '-0.5', '10', 'true', '3'], '0,1,3,2,2,0.10,1,2')]
        flags = ['--window-mode', '--aspect-ratio', '--render-scale', '--antialiasing',
                 '--texture-lod', '--sharpening-percent', '--widescreen-hud', '--menu-hud-scale']
        for values, expected in matrix:
            saved.write_bytes(saved_bytes)
            environment = {k: v for k, v in os.environ.items() if not k.startswith('RECOMP_')}
            # Deliberately conflict with argv: argv must win over inherited values.
            environment.update(zip(keys, ['1', '0', '1', 'off', '0', '0', '0', 'off']))
            environment.update(RECOMP_SETTINGS=str(saved), GAMEPKG_CHECK_LAUNCH='1',
                               GAMEPKG_EXPECT_LAUNCH=expected, ASAN_OPTIONS='detect_leaks=0')
            native_args = [part for pair in zip(flags, values) for part in pair]
            run([str(executable), *native_args], cwd=work, env=environment)
            assert saved.read_bytes() == saved_bytes, 'Launch overrides changed saved in-game settings'
        invalid = [('--render-scale', v) for v in ['0', '5', '-1', '3x', '3;touch', '']] + [
            ('--menu-hud-scale', '0'), ('--sharpening-percent', '101'),
            ('--sharpening-percent', '-1'), ('--sharpening-percent', 'nan'),
            ('--antialiasing', 'anything'), ('--texture-lod', '-100'),
            ('--widescreen-hud', 'yes'), ('--aspect-ratio', '32:9')]
        for pair in invalid:
            assert subprocess.run([str(executable), *pair], cwd=work, env=environment,
                                  capture_output=True).returncode == 2, pair
        for flag in flags:
            assert subprocess.run([str(executable), flag], cwd=work, env=environment,
                                  capture_output=True).returncode == 2, flag
        print(json.dumps({'result': 'pass-real-settings-precedence-and-graphics-defaults',
                          'native32': args.m32, 'sanitized': args.sanitize,
                          'source_mutation_guards': 'pass',
                          'launch_override_configurations': len(matrix), 'invalid_cli_values_rejected': len(invalid) + len(flags),
                          'settings_suite': result.stderr.strip()}, indent=2))


if __name__ == '__main__':
    main()
