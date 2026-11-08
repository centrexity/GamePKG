"""Maintainer-only prompt/input gate; no ROM, graphics device or player Python.

Read only the reviewed source kit's headers, two original runtime files and
host-owned glyph drawings. Compile the real staged implementation and compare
its rendered button choices against the original input mapping.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile


REVIEWED = {
    '3157cadcbc5b7c397238821035d1b172cd4edebbcb4ce2bf2f6bca9b038e4516',
    'db2a7ddada4d04a7691501480a5a15f2c71be43fc31921f033f8ca292eb0913f',
    'd94c9bdb3a5e49733ebbe78c573e56174b00ca7ffb83ffbfd9fd0fa091c0905b',
    '8ae26ce68862dffde427315fc335e2e6effa1d26ecd9ad02778499c4d478a7d0',
    'd11056c7b8e1306023b0229dd32f6afb60194fd786b73e3abf187210eeb0de5f',
    '7f56741fa8f0a56d3e718af1f772bae9200514b6293c2d218e56cde700123c8b',
    'ab30c4e7a4a86580a6fbefe7f79be941c713a0dd2b089d73fba6180729ab845b',
    '18afa3a1f2a0e9ddad89a33c2a3e33803e0764be0035da5e2c7a2e1a5235c885',
    '7cfd18d26b8a6e51005f75d7cedc1c01925237574bc579df5027b0167413a986',
}


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--kit', type=Path, default=root / 'ports/takedawn/assets/takedawn-source-kit.tar.gz')
    parser.add_argument('--cc', default=os.environ.get('CC', 'cc'))
    parser.add_argument('--m32', action='store_true',
                        help='Compile and execute the input/glyph corpus as 32-bit processes')
    parser.add_argument('--glyphs-only', action='store_true', help='Run the prompt renderer/mapping corpus without the separate legacy gameplay-hook fixture')
    parser.add_argument('--system-include', type=Path, action='append', default=[])
    args = parser.parse_args()
    digest = hashlib.file_digest(args.kit.open('rb'), 'sha256').hexdigest()
    if digest not in REVIEWED:
        raise RuntimeError('Unreviewed source kit')
    with tempfile.TemporaryDirectory(prefix='gamepkg-input-glyphs.') as temporary:
        work = Path(temporary)
        selected = {}
        with tarfile.open(args.kit, 'r:gz') as archive:
            manifest = archive.next()
            assert manifest.name == 'kit.json' and manifest.size < 4 * 1024 * 1024
            inventory = json.load(archive.extractfile(manifest))['files']
            for item in archive:
                name = item.name
                wanted = (((name.startswith('toolkit/')
                            or name.startswith('game/src/game/recomp/gen/'))
                           and name.endswith('.h'))
                          or name in {'game/src/game/glyphs.c', 'game/src/game/glyphs.h',
                                      'game/src/game/controls.c', 'game/src/game/controls.h',
                                      'glyphs/bo3_glyph_art.c', 'glyphs/bo3_glyph_art.h'})
                if not wanted:
                    continue
                assert item.isfile() and item.size <= 16 * 1024 * 1024
                assert not Path(name).is_absolute() and '..' not in Path(name).parts
                data = archive.extractfile(item).read()
                assert hashlib.sha256(data).hexdigest() == inventory[name]['sha256']
                target = work / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(data)
                selected[name] = data
        bridge = root / 'engines/takedawn/media-bridge'
        for kind in ['glyphs', 'controls']:
            original = work / f'game/src/game/{kind}.c'
            staged = work / f'{kind}.c'

            def stage(source, output, success):
                result = subprocess.run(['cmake', f'-DINPUT={source}', f'-DOUTPUT={output}',
                                         f'-DKIND={kind}', '-P', str(bridge / 'adapt_input_glyphs.cmake')],
                                        capture_output=True, text=True)
                if (result.returncode == 0) != success:
                    raise RuntimeError(result.stdout + result.stderr)

            stage(original, staged, True)
            stage(original, original, False)
            changed = work / 'changed.c'
            changed.write_bytes(original.read_bytes() + b'\n/* unreviewed */\n')
            rejected = work / 'must-not-exist.c'
            stage(changed, rejected, False)
            assert not rejected.exists() and original.read_bytes() == selected[f'game/src/game/{kind}.c']
        includes = [work / path for path in ['game/src/game', 'game/src/game/recomp/gen',
                                            'toolkit/src', 'toolkit/src/input',
                                            'toolkit/src/kernel', 'glyphs']]
        includes.append(bridge / 'input-prompts')
        common = [args.cc, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                  '-Wno-missing-field-initializers', *[f'-I{path}' for path in includes]]
        for path in args.system_include:
            common += ['-idirafter', str(path.resolve())]
        if args.m32:
            common += ['-m32', '-DGAMEPKG_EXPECT_POINTER_BITS=32']
        subprocess.run([*common, '-c', str(work / 'glyphs.c'), '-o', str(work / 'no-art.o')], check=True)
        subprocess.run([*common, '-fsanitize=address,undefined', '-fno-omit-frame-pointer',
                        '-DBO3_HAVE_GLYPH_ART', f'-DGAMEPKG_GLYPHS_SOURCE="{work / "glyphs.c"}"',
                        str(bridge / 'tests/input_glyphs.c'), str(work / 'glyphs/bo3_glyph_art.c'),
                        *[str(p) for p in sorted((bridge / 'input-prompts/generated').glob('prompt_art*.c'))],
                        '-o', str(work / 'check')], check=True)
        subprocess.run([str(work / 'check'), str(work / 'prompts.ppm')], check=True,
                       env=dict(os.environ, ASAN_OPTIONS='detect_leaks=0'))
        if args.glyphs_only:
            print('PASS: prompt renderer, original button mapping, high-resolution art, no-art build and source guards; separate gameplay-hook fixture not requested')
            return
        # Compile the complete staged input hook, not just the pure button table.
        # Unused runtime callbacks are discarded; the gate reads synthetic RAM.
        subprocess.run([*common, '-fsanitize=address,undefined', '-fno-omit-frame-pointer',
                        '-ffunction-sections', '-fdata-sections', '-Wl,--gc-sections',
                        f'-DGAMEPKG_CONTROLS_SOURCE="{work / "controls.c"}"',
                        str(bridge / 'tests/input_gameplay.c'),
                        '-o', str(work / 'gameplay-check')], check=True)
        subprocess.run([str(work / 'gameplay-check')], check=True,
                       env=dict(os.environ, ASAN_OPTIONS='detect_leaks=0'))
    print('PASS: original input/render agreement, player assignments, driving/menu transitions, all glyph families, no-art build, source and in-place rejection; ASan/UBSan clean')


if __name__ == '__main__':
    main()
