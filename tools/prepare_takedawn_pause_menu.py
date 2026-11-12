"""Adapt copied maintainer drivers to the native port's optional pause-menu row.

The game and upstream control assertions stay unchanged. Diagnostic logging
records when the native menu inserts Takedawn Settings immediately before Quit.
"""
import argparse
from pathlib import Path

ORIGINAL = '    for p in down:150 down:150 down:150 down:150 a:300; do press "$p" 0; done'
REPLACEMENT = '''    gamepkg_quit_steps=$(scene_pause_quit_downs)
    for (( gamepkg_quit_i=0; gamepkg_quit_i<gamepkg_quit_steps; gamepkg_quit_i++ )); do
        press down:150 0
    done
    press a:300 0'''
SELECTOR = '''
# The native port inserts one Settings row before the original Quit action.
# These diagnostic drivers already require the full runtime log for navigation.
scene_pause_quit_downs() {
    if grep -aqF '[MENU] pause: TAKEDAWN SETTINGS listed' "$D/log.txt"; then
        echo 5
    else
        echo 4
    fi
}
'''

def prepare(scene_lib, controls):
    if (scene_lib.name != 'scene_lib.sh' or controls.name != 'controls_scene.sh'
            or scene_lib.parent.resolve() != controls.parent.resolve()
            or scene_lib.is_symlink() or controls.is_symlink()):
        raise RuntimeError('expected two regular copied drivers in the same directory')
    library, script = scene_lib.read_text(), controls.read_text()
    if 'scene_pause_quit_downs()' in library or script.count(ORIGINAL) != 1:
        raise RuntimeError('unsupported or already adapted pause driver')
    updated = script.replace(ORIGINAL, REPLACEMENT)
    scene_lib.write_text(library + SELECTOR)
    controls.write_text(updated)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('scene_lib', type=Path)
    parser.add_argument('controls', type=Path)
    args = parser.parse_args()
    prepare(args.scene_lib, args.controls)

if __name__ == '__main__':
    main()
