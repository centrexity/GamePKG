"""Asset-free check of the reviewed native window-failure policy."""
from pathlib import Path
import os
import subprocess
import tempfile

root = Path(__file__).resolve().parent
patch = (root.parent / 'patches/0013-gamepkg-require-game-window.patch').read_text()
section = patch.split('diff --git a/port/linux/src/gamepkg_video_failure.inc ')[1].split('diff --git ')[0]
header = '\n'.join(line[1:] for line in section.splitlines()
                   if line.startswith('+') and not line.startswith('+++')) + '\n'
assert '+\t\t\tgamepkg_video_failure();' in patch
fixture = r'''
#include <stdio.h>
static int config_boolean(const char *name) {
    (void)name;
    return getenv("TEST_NULL_RENDERER") != NULL;
}
static void platform_log(const char *message) { fprintf(stderr, "%s\n", message); }
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-window-policy-') as temporary:
    folder = Path(temporary)
    source = folder / 'fixture.c'
    source.write_text('#include <stdlib.h>\n' + fixture + header +
                      '\nint main(void) { gamepkg_video_failure(); return 0; }\n')
    binary = folder / 'fixture'
    subprocess.run(['cc', '-std=c11', '-O2', '-Wall', '-Wextra', '-Werror',
                    str(source), '-o', str(binary)], check=True, timeout=30)
    for required, headless, expected in [(None, False, 0), ('0', False, 0),
                                        ('1', False, 1), ('1', True, 0)]:
        env = dict(os.environ)
        env.pop('GAMEPKG_REQUIRE_WINDOW', None)
        env.pop('TEST_NULL_RENDERER', None)
        if required is not None:
            env['GAMEPKG_REQUIRE_WINDOW'] = required
        if headless:
            env['TEST_NULL_RENDERER'] = '1'
        result = subprocess.run([str(binary)], env=env, capture_output=True,
                                text=True, timeout=10)
        assert result.returncode == expected, (required, headless, result)
        assert ('unable to create the game window' in result.stderr) == (expected == 1)
print('Native window failure: normal launch fails clearly; explicit headless and standalone policies preserved')
