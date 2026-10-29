"""Asset-free native interrupt/cache gate over the pinned original kernel.

Uses a private copy and original IRQ regressions plus native Rust ABI cases.
Does not read an ISO, run a game, or open an audio/video output device.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from takedawn_native_privileged import adapt, patch_kernel, receipt, PAIR, PAIR_NATIVE, CACHE, ADAPTED_UNITS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--prefix', type=Path)
    parser.add_argument('--without-vulkan', action='store_true',
                        help='IRQ-only fixture; real-game Vulkan qualification stays separate')
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    updated, seen = adapt(PAIR)
    assert seen == ADAPTED_UNITS['recomp_0023_018'] and PAIR_NATIVE in updated
    assert updated.index('esi = eax;') < updated.index('    gamepkg_takedawn_sti_after_shadow();')
    updated, seen = adapt(CACHE)
    assert seen == ADAPTED_UNITS['recomp_0023_011'] and 'gamepkg_takedawn_wbinvd();' in updated
    for bad in [PAIR + PAIR, CACHE + CACHE, PAIR.replace('    esi = eax;', ''), PAIR.replace('    eax = MEM32(ecx + 8);', '    eax = 0;')]:
        try:
            adapt(bad)
        except RuntimeError:
            pass
        else:
            raise AssertionError('duplicate or incomplete original critical region accepted')
    assert adapt('unrelated code') == ('unrelated code', [])
    try:
        receipt(project, {'recomp_0023_018': ['00367C86']})
    except RuntimeError:
        pass
    else:
        raise AssertionError('incomplete adaptation receipt accepted')
    work = Path(tempfile.mkdtemp(prefix='gamepkg-native-privileged.'))
    print('Private evidence:', work, flush=True)
    toolkit = work / 'toolkit'
    shutil.copytree(args.toolkit, toolkit, ignore=shutil.ignore_patterns('.git'))
    kernel = toolkit / 'src/kernel/kernel_hal.c'
    original = kernel.read_text()
    implementation = (project / 'engines/takedawn/rust-port/native_irq.inc.h').read_text()
    patched = patch_kernel(original, implementation)
    try:
        patch_kernel(original + '\n', implementation)
    except RuntimeError:
        pass
    else:
        raise AssertionError('unreviewed kernel accepted')
    kernel.write_text(patched)
    archive = work / ('native.lib' if os.name == 'nt' else 'libnative.a')
    with (work / 'rust.log').open('w') as log:
        subprocess.run(['rustc', '--edition=2021', '--crate-type=staticlib', '-Dwarnings', '-Copt-level=3', '-Cpanic=abort', str(project / 'engines/takedawn/rust-port/native_privileged.rs'), '-o', str(archive)], check=True, stdout=log, stderr=subprocess.STDOUT, timeout=120)
    source = toolkit / 'tests/irq_safe_points'
    shutil.copy2(project / 'engines/takedawn/rust-port/tests/native_irq_cases.c', source / 'gamepkg_native_irq_cases.c')
    main_source = source / 'test_main.c'
    text = main_source.read_text()
    assert text.count('int main(void)\n') == 1 and text.count('    printf("irq_safe_points:\\n");') == 1
    text = text.replace('int main(void)\n', '#include "gamepkg_native_irq_cases.c"\n\nint main(void)\n')
    text = text.replace('    printf("irq_safe_points:\\n");', '    native_mask_cases();\n    native_cache_alias_case();\n    printf("irq_safe_points:\\n");')
    main_source.write_text(text)
    cmake = source / 'CMakeLists.txt'
    text = cmake.read_text()
    text += '\ntarget_link_libraries(irq_safe_points_test PRIVATE "' + archive.as_posix() + '")\n'
    if os.name != 'nt':
        text += '\nset(CMAKE_C_LINK_EXECUTABLE "<CMAKE_C_COMPILER> <FLAGS> <CMAKE_C_LINK_FLAGS> <LINK_FLAGS> <OBJECTS> -o <TARGET> -Wl,--start-group <LINK_LIBRARIES> -Wl,--end-group")\n'
    else:
        text += '\ntarget_link_libraries(irq_safe_points_test PRIVATE ws2_32 userenv ntdll bcrypt advapi32 user32 kernel32)\n'
    cmake.write_text(text)
    build = work / 'build'
    configure = ['cmake', '-S', str(source), '-B', str(build), '-G', 'Ninja', '-DCMAKE_BUILD_TYPE=Release']
    if args.without_vulkan:
        configure += ['-DXBOXRECOMP_VULKAN=OFF']
    if args.prefix:
        prefix = args.prefix.resolve()
        configure += ['-DCMAKE_PREFIX_PATH=' + str(prefix)]
        if os.name != 'nt':
            configure += ['-DCMAKE_EXE_LINKER_FLAGS=-L' + str(prefix / 'lib/x86_64-linux-gnu')]
    with (work / 'configure.log').open('w') as log:
        subprocess.run(configure, check=True, stdout=log, stderr=subprocess.STDOUT, timeout=120)
    results = []

    def run(label, should_fail=None):
        with (work / (label + '-build.log')).open('w') as log:
            subprocess.run(['cmake', '--build', str(build), '--target', 'irq_safe_points_test', '-j', '4'], check=True, stdout=log, stderr=subprocess.STDOUT, timeout=300)
        result = subprocess.run([str(build / ('irq_safe_points_test.exe' if os.name == 'nt' else 'irq_safe_points_test'))], env=dict(os.environ, SDL_AUDIODRIVER='dummy'), text=True, capture_output=True, timeout=60)
        text = result.stdout + result.stderr
        (work / (label + '.log')).write_text(text)
        if should_fail:
            assert result.returncode != 0 and any(should_fail in line and 'FAIL' in line for line in text.splitlines()), text
        else:
            assert result.returncode == 0 and 'all passed' in text and 'FAIL' not in text, text
        results.append({'case': label, 'exit': result.returncode, 'expected_failed_assertion': should_fail})
        print(label, 'PASS' if not should_fail else 'required failure observed', flush=True)

    run('native')
    # Mutations must fail the named behavioral invariant, not just fail to link.
    kernel.write_text(patched.replace('if (t_native_irq_masked || t_in_isr || g_current_irql > DISPATCH_LEVEL)', 'if (t_in_isr || g_current_irql > DISPATCH_LEVEL)', 1))
    run('missing-safe-point-mask', 'safe point inside CLI does not deliver')
    kernel.write_text(patched.replace('if (!t_native_irq_masked && gate_try())', 'if (gate_try())'))
    run('recursive-post-bypass', 'recursive same-thread post cannot enter ISR')
    kernel.write_text(patched)
    run('restored-native')
    (work / 'result.json').write_text(json.dumps({'cases': results, 'implementation': receipt(project, ADAPTED_UNITS), 'patched_kernel_sha256': hashlib.sha256(patched.encode()).hexdigest(), 'owned_media_required': False, 'runtime_instruction_decoder': False}, indent=2) + '\n')
    print('PASS: original IRQ suite, native Rust cache/interrupt ABI, shadow ordering and targeted mutation checks.')


if __name__ == '__main__':
    main()
