"""Execute the pinned SDL2 backend, original input tests and shared Rust service.

Maintainer verification only. Uses virtual controllers and silent dummy outputs;
refuses physical devices. Does not establish real-controller delivery or FPS.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PINNED = {
    'src/input/xinput_device.c': '316fe27024ec06ccf72cd6cd08b8602de87dd94843e2be7f99ad243eae47ae9c',
    'src/input/input_core.c': '6ea3903d962aea341e91b7acf775e21db5f3e6d24177586e5169a1bc1eff9aa8',
    'src/input/input_map.c': 'ca12f7720091322e3f125bf38659f5f2877d1fb3f209fcca3c9f818231e0ba8b',
    'tests/input_rumble_sdl/test_main.c': '7618a3287b9362de5aa96dc6ff76016d42e1c76afc0c7c7afae9a244f8602a2d',
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--library', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--cc')
    parser.add_argument('--cflags', default='')
    parser.add_argument('--sdl2-library', type=Path)
    parser.add_argument('--sdl2-include', type=Path, default=Path('/usr/include/SDL2'))
    args = parser.parse_args()
    sdk, library, out = args.toolkit.resolve(), args.library.resolve(), args.output.resolve()
    assert library.is_file(), library
    assert all(hashlib.sha256((sdk / name).read_bytes()).hexdigest() == digest
               for name, digest in PINNED.items()), 'unreviewed SDK input'
    out.mkdir(parents=True, exist_ok=True)
    # Negative controls intentionally abort. Do not leave crash dumps behind.
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    env = dict(os.environ, SDL_VIDEODRIVER='dummy', SDL_AUDIODRIVER='dummy')

    def run(command, name, expected=None, cwd=ROOT):
        result = subprocess.run([str(item) for item in command], cwd=cwd, env=env,
                                capture_output=True, text=True, timeout=180)
        (out / (name + '.log')).write_text(result.stdout + result.stderr)
        if expected:
            assert result.returncode != 0 and 'Assertion' in result.stderr and expected in result.stderr, result.stderr
        else:
            assert result.returncode == 0, name + ': ' + result.stderr[-2000:]
        return result

    bridge = ROOT / 'engines/takedawn/media-bridge'
    adapter = bridge / 'adapt_native_sdl2.cmake'
    original_path = sdk / 'src/input/xinput_device.c'
    changed = out / 'changed-input.c'
    changed.write_bytes(original_path.read_bytes() + b'\n/* changed pinned input */\n')
    rejected = out / 'must-not-exist.c'
    rejected.unlink(missing_ok=True)
    for source, destination, marker in [
        (changed, rejected, 'Unreviewed native controller source'),
        (out / 'inplace-input.c', out / 'inplace-input.c', 'Never overwrite pinned input source'),
    ]:
        if source == destination:
            source.write_bytes(original_path.read_bytes())
        result = subprocess.run(['cmake', '-DINPUT=' + str(source), '-DOUTPUT=' + str(destination),
                                 '-P', str(adapter)], capture_output=True, text=True, timeout=30)
        (out / ('reject-inplace.log' if source == destination else 'reject-modified.log')).write_text(result.stdout + result.stderr)
        assert result.returncode != 0 and marker in result.stderr
    assert not rejected.exists()
    assert (out / 'inplace-input.c').read_bytes() == original_path.read_bytes()
    headers = out / 'negative-headers'
    headers.mkdir(exist_ok=True)
    header = (bridge / 'native_sdl2.h').read_text().replace(
        '../../../include/gamepkg_input_backend.h', str(ROOT / 'include/gamepkg_input_backend.h'))
    for name, before, after in [('buttons', '12,13,14,15', '13,12,14,15'),
                                ('y', '-1 - (int)value', '-(int)value')]:
        assert header.count(before) == 1
        (headers / (name + '.h')).write_text(header.replace(before, after))
    build = out / 'build'
    command = ['cmake', '-S', ROOT / 'tools/fixtures/native-sdl2', '-B', build, '-G', 'Ninja',
               '-DSDK=' + str(sdk), '-DBRIDGE=' + str(bridge),
               '-DGAMEPKG_TAKEDAWN_MEDIA_ARCHIVE=' + str(library),
               '-DGAMEPKG_NEGATIVE_HEADERS=' + str(headers)]
    if args.cc:
        command += ['-DCMAKE_C_COMPILER=' + args.cc]
    if args.cflags:
        command += ['-DCMAKE_C_FLAGS=' + args.cflags]
    if args.sdl2_library:
        command += ['-DGAMEPKG_TEST_SDL2_LIBRARY=' + str(args.sdl2_library.resolve()),
                    '-DGAMEPKG_TEST_SDL2_INCLUDE=' + str(args.sdl2_include.resolve())]
    run(command, 'configure')
    targets = ['gamepkg_native_sdl2', 'input_map_test', 'input_rumble_sdl_test', 'negative_buttons', 'negative_y']
    run(['cmake', '--build', build, '--target', *targets, '--parallel', '4'], 'build')
    for mode in ['', '--window-pump']:
        result = run([build / 'gamepkg_native_sdl2', *([mode] if mode else [])], 'window' if mode else 'headless')
        assert 'PASS native SDL2/Rust' in result.stdout
    result = run([build / 'input/input_map/input_map_test', sdk / 'src/input/README.md'], 'original-mapping')
    assert '682 checks, 0 failed' in result.stdout
    result = run([build / 'input/input_rumble_sdl/input_rumble_sdl_test'], 'original-rumble')
    assert '0 failed' in result.stdout
    run([build / 'negative_buttons'], 'negative-buttons', 'report.buttons==expected[bit]')
    run([build / 'negative_y'], 'negative-y', 'report.left_x==axes[0]')
    original = (sdk / 'src/input/xinput_device.c').read_text()
    staged = (build / 'gamepkg-media/xinput_device.c').read_text()
    marker = '#else /* !_WIN32 */'
    assert original.split(marker)[0] == staged.split(marker)[0], 'Windows backend changed'
    assert all(hashlib.sha256((sdk / name).read_bytes()).hexdigest() == digest
               for name, digest in PINNED.items()), 'pinned SDK modified'
    receipt = {'result': 'pass', 'library': str(library), 'sdk_inputs': PINNED,
               'scope': 'Actual SDL2 backend/core and Rust service, virtual controllers only; original mapping and rumble assertions with one explicit virtual join press; 2 failing negative controls. No physical input, gameplay or FPS claim.'}
    (out / 'result.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(receipt['scope'])


if __name__ == '__main__':
    main()
