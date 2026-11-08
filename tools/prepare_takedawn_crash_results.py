"""Wait for the native Crash Results replay before the original Continue press.

Maintainer driver adaptation only. The game and original input assertions are
unchanged. p1_results rises before the hand-over/loading screen; the background
replay then counts down and enters phase 2 while p1_results remains set.
"""
import argparse
from pathlib import Path

ORIGINAL = '''    wait_ctl 'p1_results 00000000 -> 00000001' 1 "results"
    wait_flips 300'''
REPLACEMENT = '''    wait_ctl 'p1_results 00000000 -> 00000001' 1 "results"
    scene_wait_crash_results_replay
    wait_flips 60'''
HELPER = '''
# A result flag alone does not mean the original Continue menu accepts input:
# the title loads and counts down its replay behind Crash Results first.
scene_wait_crash_results_replay() {
    local began=$(date +%s) ready
    while true; do
        ready=$(awk '
            $1 == "S" && $3 == "p1_results" { active = $6 == "00000001"; if (active && !result) result = $2 }
            $1 == "S" && active && result && $2 > result && $3 == "p1_phase" && $6 == "00000002" { print $2; exit }
        ' "$D/ctl.txt" 2>/dev/null)
        if [[ "$ready" =~ ^[0-9]+$ ]]; then
            note "Crash Results background replay active (flip $ready)"
            return
        fi
        alive
        if (( $(date +%s) - began > SCENE_TIMEOUT )); then
            die "Crash Results replay did not finish loading"
        fi
        sleep 0.2
    done
}
'''


def prepare(library, driver):
    if (library.name != 'scene_lib.sh' or driver.name != 'controls_scene.sh'
            or library.parent.resolve() != driver.parent.resolve()
            or library.is_symlink() or driver.is_symlink()):
        raise RuntimeError('expected regular copied scene drivers in the same directory')
    lib, scene = library.read_text(), driver.read_text()
    if 'scene_wait_crash_results_replay()' in lib or scene.count(ORIGINAL) != 1:
        raise RuntimeError('unsupported or already adapted Crash Results driver')
    library.write_text(lib + HELPER)
    driver.write_text(scene.replace(ORIGINAL, REPLACEMENT))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('scene_lib', type=Path)
    parser.add_argument('controls', type=Path)
    args = parser.parse_args()
    prepare(args.scene_lib, args.controls)


if __name__ == '__main__':
    main()
