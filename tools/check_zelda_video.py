"""Qualify original Zelda widescreen/tall rendering through both runtime paths.

Maintainer-only. The selected ROM stays in place; generated recipes, signing
key, libraries and frame fingerprints stay in a private temporary directory.
"""
from pathlib import Path
import argparse
import os
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--rom', type=Path, required=True)
parser.add_argument('--sysroot', type=Path, required=True)
parser.add_argument('--slang-root', type=Path, required=True)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-zelda-video-') as temporary:
    root = Path(temporary)
    settings = root / 'settings.toml'
    settings.write_text('automatic_update_checks=false\n')
    environment = dict(os.environ, GAMEPKG_CONFIG=str(settings),
        GAMEPKG_SLANG_ROOT=str(args.slang_root.resolve()),
        GAMEPKG_ZELDA3_SYSROOT=str(args.sysroot.resolve()),
        GAMEPKG_ZELDA3_TEST_ROM=str(args.rom.resolve(strict=True)),
        GAMEPKG_BUNDLED_AUDIO_DECODER=str(project / 'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'),
        GAMEPKG_BUNDLED_AUDIO_CONVERTER=str(project / 'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'))
    cli = project / 'target/release/gamepkg'
    def run(*command):
        subprocess.run(list(map(str, command)), cwd=project, env=environment, check=True, timeout=600)
    def test(name):
        run('cargo', 'test', '--locked', '--release', '--all-features', '--test', 'zelda3_play', name, '--', '--ignored', '--exact')
    run(cli, 'keygen', '-o', root / 'creator.key')
    baseline = None
    cases = [(None, False)] + [(extra, tall) for extra in [0, 1, 32, 96] for tall in [False, True]]
    for index, (extra, tall) in enumerate(cases):
        recipe = root / f'recipe{index}'
        shutil.copytree(project / 'ports/zelda3', recipe)
        if extra is not None:
            with (recipe / 'manifest.toml').open('a') as manifest:
                manifest.write(f'\n[zelda_video]\nextra_width = {extra}\nextend_height = {str(tall).lower()}\n')
        library = root / f'library{index}'
        environment.update(GAMEPKG_HOME=str(library), GAMEPKG_ZELDA3_TEST_ROOT=str(library),
            GAMEPKG_ZELDA3_VIDEO_WIDTH=str(256 + 2 * (extra or 0)), GAMEPKG_ZELDA3_VIDEO_HEIGHT=str(240 if tall else 224))
        package = root / f'video{index}.gamepkg'
        run(cli, 'pack', recipe, '-o', package, '--key', root / 'creator.key')
        run(cli, 'install', package, '--trust-creator')
        fingerprints = []
        for target in ['native', 'wasm']:
            run(cli, 'build', 'com.gamepkg.zelda3', '--target', target)
            output = root / f'fingerprint-{index}-{target}'
            environment.update(GAMEPKG_ZELDA3_TEST_TARGET=target, GAMEPKG_ZELDA3_VIDEO_FINGERPRINT=str(output))
            test('actual_zelda3_video_dimensions_aspect_and_pixels')
            fingerprints.append(output.read_text())
            if extra is None or extra == 96:
                for name in ['actual_zelda3_frames', 'actual_zelda3_stereo_title_preserves_channel_differences', 'actual_zelda3_opposing_directions_follow_original_filter']:
                    test(name)
        assert fingerprints[0] == fingerprints[1], f'Native/Wasm pixels differ: extra={extra}, tall={tall}'
        if extra is None:
            baseline = fingerprints[0]
        elif extra == 0 and not tall:
            assert fingerprints[0] == baseline, 'Explicit default changed original pixels'
        print(f'Video extra={extra}, tall={tall}: dimensions/aspect, 1,000 frames and exact native/Wasm pixel fingerprints passed.', flush=True)
