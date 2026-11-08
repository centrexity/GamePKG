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
        checks = '\n'.join(
            f'    CHECK(xbox_SettingGet({key})->builtin == {value});'
            for key, value in [('XS_GRAPHICS_RENDER_SCALE', '2'),
                               ('XS_DISPLAY_ASPECT', '1'), ('XS_DISPLAY_HUD_FIX', '1'),
                               ('XS_GRAPHICS_AA', '2'), ('XS_GRAPHICS_SHARPEN', '0.10'),
                               ('XS_GRAPHICS_LOD_BIAS', '1'), ('XS_GRAPHICS_UI_SCALE', '1')])
        test = test.replace('    xbox_SettingsInit("Test");',
                            '    xbox_SettingsInit("Test");\n' + checks)
        fixture = work / 'test_main.c'
        fixture.write_text(test)
        command = [args.cc, '-std=c11', '-D_POSIX_C_SOURCE=200809L',
                   '-DRECOMP_ENV_HAVE_ENHANCE_KEYS', '-O2', '-Wall', '-Wextra']
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
        print(json.dumps({'result': 'pass-real-settings-precedence-and-graphics-defaults',
                          'native32': args.m32, 'sanitized': args.sanitize,
                          'source_mutation_guards': 'pass',
                          'settings_suite': result.stderr.strip()}, indent=2))


if __name__ == '__main__':
    main()
