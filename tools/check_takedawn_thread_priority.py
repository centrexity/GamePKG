"""Maintainer-only KTHREAD staging, source-integrity and native handle tests.

No game assets; Python is used only by maintainers/CI, never by players.
"""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile


EXPECTED_SOURCE = 'bf62f8787a2e78a0abdb28e10ad8f79c5267dc655491353af231067747ce446e'
EXPECTED_REVISION = '5ae8ad90d984425e834dee759592c15592679880'


def reference_bytes(source, checkout):
    if hashlib.sha256(checkout).hexdigest() == EXPECTED_SOURCE:
        return checkout  # Includes the canonical files in an extracted source kit.
    # A Windows checkout can use CRLF. Stage the canonical pinned Git object,
    # preserving both the source fingerprint and the checkout's exact bytes.
    revision = subprocess.check_output(['git', '-C', str(source), 'rev-parse', 'HEAD'], text=True).strip()
    if revision != EXPECTED_REVISION:
        raise RuntimeError('Unreviewed original runtime revision')
    canonical = subprocess.check_output(['git', '-C', str(source), 'show', 'HEAD:src/kernel/kernel_bridge.c'])
    if hashlib.sha256(canonical).hexdigest() != EXPECTED_SOURCE:
        raise RuntimeError('Unreviewed kernel bridge source')
    if checkout != canonical.replace(b'\n', b'\r\n'):
        raise RuntimeError('Checkout differs beyond Windows line endings')
    return canonical


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--target', default='x86_64-pc-windows-msvc' if os.name == 'nt'
                        else 'x86_64-pc-windows-gnu', choices=['x86_64-pc-windows-msvc',
                        'i686-pc-windows-msvc', 'x86_64-pc-windows-gnu', 'i686-pc-windows-gnu'])
    parser.add_argument('--compile-only', action='store_true', help='Cross build; run fixture on actual Windows separately')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    bridge = root / 'engines/takedawn/media-bridge'
    source = args.source.resolve()
    upstream = source / 'src/kernel/kernel_bridge.c'
    checkout_before = upstream.read_bytes()
    before = reference_bytes(source, checkout_before)
    work = Path(tempfile.mkdtemp(prefix='gamepkg-thread-priority.'))
    original = work / 'original.c'
    original.write_bytes(before)
    staged = work / 'kernel_bridge.c'

    def stage(src, dest, succeeds):
        result = subprocess.run(['cmake', '-DINPUT=' + str(src), '-DOUTPUT=' + str(dest),
                                 '-P', str(bridge / 'adapt_threads.cmake')], capture_output=True)
        assert (result.returncode == 0) == succeeds, result.stderr.decode(errors='replace')

    stage(original, staged, True)
    text = staged.read_text()
    prefix = '/* GamePKG: resolve guest thread objects before host priority calls. */\n'
    assert text.startswith(prefix)
    text = text[len(prefix):]
    cpu = '#if defined(_WIN32)\n' + (bridge.parent / 'rust-port/native_windows_cpu.inc.h').read_text() + '\n#endif\n'
    assert text.count(cpu) == 1, 'Missing or duplicated reviewed CPU policy'
    text = text.replace(cpu, '')
    role = 'void xbox_log_thread_role(const char *role, uint32_t routine)\n{\n'
    routed_role = role + '#if defined(_WIN32)\n    if (role && !strcmp(role, "nv2a-ack"))\n        gamepkg_windows_pin_executor();\n#endif\n'
    assert text.count(routed_role) == 1, 'Missing or duplicated executor CPU routing'
    text = text.replace(routed_role, role)
    helper = (bridge / 'thread_priority.inc.h').read_text() + '\n'
    assert text.count(helper) == 1
    text = text.replace(helper, '')
    start = 'static void bridge_KeQueryBasePriorityThread(void)'
    # Only these two wrappers and their preceding resolver declaration change.
    old_start = before.decode().index(start)
    old_end = before.decode().index('/* ── KeStallExecutionProcessor', old_start)
    new_start = text.index('/* Ke* takes a guest KTHREAD')
    new_end = text.index('/* ── KeStallExecutionProcessor', new_start)
    changed = text[new_start:new_end]
    assert 'XBOX_TO_NATIVE' not in changed and changed.count('bridge_priority_thread(STACK_ARG(0), &owned)') == 2
    assert changed.count('if (owned) CloseHandle(thread);') == 2
    text = text[:new_start] + before.decode()[old_start:old_end] + text[new_end:]
    assert text == before.decode(), 'Unrelated original kernel code changed'
    stage(original, original, False)
    altered = work / 'altered.c'
    altered.write_bytes(before + b'\n/* changed source */\n')
    rejected = work / 'must-not-exist.c'
    stage(altered, rejected, False)
    assert not rejected.exists()
    assert upstream.read_bytes() == checkout_before and original.read_bytes() == before
    exe = work / 'thread-priority.exe'
    fixture = bridge / 'check_thread_priority.c'
    if args.target.endswith('-msvc'):
        command = ['cl', '/nologo', '/std:c11', '/O2', '/W4', '/WX',
                   str(fixture), '/Fe:' + str(exe)]
    else:
        cc = args.target.replace('pc-windows-gnu', 'w64-mingw32') + '-gcc'
        command = [cc, '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror', '-static',
                   str(fixture), '-o', str(exe)]
    subprocess.run(command, cwd=work, check=True)
    if not args.compile_only:
        subprocess.run([str(exe)], check=True)
    print('PASS: only reviewed priority wrappers/helper and Windows executor CPU policy changed; altered/in-place source rejected; original unchanged')
    print('Fixture:', exe)
    print('Execution:', 'pending actual Windows' if args.compile_only else 'passed')


if __name__ == '__main__':
    main()
