"""Asset-free checks of the patched engine's actual co-op clock, handoff and view functions."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from check_scripted_suspension import block

HERE = Path(__file__).resolve().parent
CLOCK_CASES = ['ahead','window','age_ping','invalid_ping','wrap','bounds','notclient','normal60','normal120','partial','held_start','hold_release','menu','host','fixedlocal','inactive','properties','catchup60','catchup120','catchup_rate','catchup_zero','catchup_stale','catchup_invalid_ping','catchup_wrap','catchup_epoch','catchup_deadband']
HANDOFF_CASES = ['silent_script','no_unit_reset','nonfinite','disabled','suspended','release','handoff','newlife','noncoop','riding','dead','bsp','oldunit','normal']


def replace_one(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--profile', choices=['native32', 'sanitized64'], required=True)
    parser.add_argument('--cc', default='clang-18')
    args = parser.parse_args()
    source = args.source.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    paths = {name: source / relative for name, relative in {
        'network': 'port/linux/game/network_distributed.c',
        'clock': 'source/game/game_time.c',
        'players': 'source/game/players.c',
        'real': 'source/math/real_math.c',
        'matrix': 'source/math/matrix_math.c',
        'version': 'port/linux/include/halo_port_limits.h',
    }.items()}
    texts = {name: path.read_text() for name, path in paths.items()}
    assert re.search(r'#define HALO_PORT_NETWORK_VERSION 34\b', texts['version'])
    network = texts['network']
    start = network.index('static boolean gamepkg_clock_slew_valid;')
    end = network.index('\nlong distributed_latest_host_time(', start)
    clock = network[start:end] + '\n' + block(texts['clock'], 'void game_time_update(')
    start = network.index('#define GAMEPKG_SCRIPT_HANDOFF_TOLERANCE')
    end = network.index('static void distributed_apply_predictions(\n', start)
    handoff = (re.search(r'^#define LOCAL_CORRECTION_TOLERANCE .+$', network, re.M)[0]
               + '\n' + network[start:end] + block(network, 'static void distributed_apply_predictions('))
    original_basis = '\n'.join([
        block(texts['real'], 'real_euler_angles2d *euler_angles2d_from_vector3d('),
        block(texts['real'], 'real_vector3d *vector3d_from_euler_angles2d('),
        block(texts['matrix'], 'void matrix4x3_rotation_from_vectors('),
        block(texts['matrix'], 'real_vector3d *matrix4x3_transform_normal('),
        block(texts['matrix'], 'real_vector3d *\nmatrix4x3_inverse_transform_normal('),
        block(texts['players'], 'void player_aiming_vector_from_facing('),
    ])
    view = block(network, 'static void gamepkg_scripted_view_local_facing(')
    no_handoff_start = handoff.index('\t\t/* port: a script')
    no_handoff_end = handoff.index('\t\tif (unit_index != NONE && object_get', no_handoff_start)
    fixtures = [
        ('clock', 'network_clock.c', clock, CLOCK_CASES, {
            'no_slew': (replace_one(clock, 'return requested + 1;', 'return requested;'), 'catchup60'),
            'no_rate': (replace_one(clock, '(!gamepkg_clock_slew_valid || (halo_u32)(now - gamepkg_clock_slew_at) >= 250)', 'TRUE'), 'catchup_rate'),
            'no_budget': (replace_one(clock, 'if (projected <= current) return 0;', 'if (projected <= current) return requested;'), 'ahead'),
            'erase_fraction': (replace_one(clock, 'if (gamepkg_clock_held && ticks_elapsed == 0)', '(void)gamepkg_clock_held;\n if (ticks_elapsed == 0)'), 'normal60'),
        }),
        ('handoff', 'scripted_handoff.c', handoff, HANDOFF_CASES, {
            'no_script_authority': (handoff[:no_handoff_start] + handoff[no_handoff_end:], 'disabled'),
        }),
        ('view_basis', 'scripted_view_basis.c', original_basis + '\n' + view, [None], {
            'parent_node_basis': (original_basis + '\nstatic void gamepkg_scripted_view_local_facing(long player_index,real_vector3d const *world,real_vector3d *local){(void)player_index;matrix4x3_inverse_transform_normal(&parent_node,world,local);}\n', None),
        }),
    ]
    flags = (['--target=i686-linux-gnu','-m32','-isystem','/usr/include/x86_64-linux-gnu']
             if args.profile == 'native32' else ['-fsanitize=address,undefined','-fno-omit-frame-pointer'])
    records = []
    for kind, fixture, production, cases, negatives in fixtures:
        work = output / kind
        work.mkdir(exist_ok=True)
        for mutation, piece, selected in [('none', production, cases), *[(name, text, [case]) for name, (text, case) in negatives.items()]]:
            (work / 'candidate.inc').write_text(piece)
            binary = work / ('check-' + mutation)
            command = [args.cc, *flags, '-std=gnu17', '-O2', '-g', '-ffp-contract=off', '-Wall', '-Wextra', '-Werror']
            if kind == 'handoff' and mutation != 'none':
                command.append('-Wno-unused-function')
            command += ['-I' + str(work), str(HERE / fixture), '-lm', '-o', str(binary)]
            build = subprocess.run(command, capture_output=True, text=True, timeout=120)
            (work / (mutation + '-build.log')).write_text(build.stdout + build.stderr)
            assert build.returncode == 0, build.stderr
            for case in selected:
                run = subprocess.run([str(binary), *([] if case is None else [case])], capture_output=True,
                                     text=True, timeout=30, env=dict(os.environ,
                                     ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',
                                     UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1'))
                (work / (mutation + '-' + (case or 'roundtrip') + '.log')).write_text(run.stdout + run.stderr)
                if mutation == 'none':
                    assert run.returncode == 0, (kind, case, run.stderr)
                    if kind == 'view_basis':
                        assert '11900 original aiming/inverse roundtrips' in run.stdout
                else:
                    assert run.returncode != 0 and 'Assertion' in run.stderr, (mutation, run.stderr)
                records.append({'fixture': kind, 'mutation': mutation, 'case': case, 'exit': run.returncode})
                label = case or ('11900 roundtrips' if mutation == 'none' else 'expected assertion rejection')
                print(args.profile, kind, mutation, label, 'PASS', flush=True)
        (work / 'candidate.inc').write_text(production)
    receipt = {'profile': args.profile, 'result': 'pass',
               'scope': 'Exact extracted engine functions; external world and device APIs mocked. Negative controls must fail their assertions. Does not establish full campaign, vehicle gameplay, Windows playback or frame-time qualification.',
               'sources': {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in paths.items()},
               'results': records}
    (output / 'result.json').write_text(json.dumps(receipt, indent=2) + '\n')


if __name__ == '__main__':
    main()
