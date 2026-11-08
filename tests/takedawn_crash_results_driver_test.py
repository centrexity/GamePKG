"""Exercise the copied-driver readiness wait with synthetic game state logs."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('results_driver', Path(__file__).resolve().parents[1]
                                               / 'tools/prepare_takedawn_crash_results.py')
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class ResultsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.library = self.root / 'scene_lib.sh'
        self.driver = self.root / 'controls_scene.sh'
        self.library.write_text('original_library=1\n')
        self.original = '# copied driver\n' + prepare.ORIGINAL + '\n    press a:300 0\n    ORIGINAL_ASSERTIONS\n'
        self.driver.write_text(self.original)
        prepare.prepare(self.library, self.driver)

    def wait(self, log):
        (self.root / 'ctl.txt').write_text(log)
        # A bounded poll without a real-time delay: a missing transition
        # must fail; a valid transition returns before alive/sleep are used.
        script = '''set -e
D="$1"; SCENE_TIMEOUT=1
source "$D/scene_lib.sh"
note() { printf '%s\\n' "$*"; }
alive() { :; }
# Fixed clock values make the first missing-state
# poll hit the deadline and exercise the actual failure path deterministically.
date() { if [[ -f "$D/time" ]]; then echo 3; else touch "$D/time"; echo 0; fi; }
die() { echo "$*" >&2; exit 9; }
scene_wait_crash_results_replay
'''
        return subprocess.run(['bash', '-c', script, 'test', str(self.root)], capture_output=True, text=True, timeout=5)

    def test_only_wait_changes_original_buttons_and_assertions_stay(self):
        text = self.driver.read_text()
        self.assertEqual(text.replace(prepare.REPLACEMENT, prepare.ORIGINAL), self.original)
        with self.assertRaises(RuntimeError):
            prepare.prepare(self.library, self.driver)

    def test_post_results_replay_phase_is_required(self):
        result = self.wait('S 100 p1_phase 00000001 -> 00000002\nS 200 p1_results 00000000 -> 00000001\nS 700 p1_phase 00000001 -> 00000002\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('flip 700', result.stdout)

    def test_stale_or_missing_replay_does_not_pass(self):
        for log in ['S 100 p1_phase 00000001 -> 00000002\nS 200 p1_results 00000000 -> 00000001\n',
                    'S 100 p1_phase 00000001 -> 00000002\n',
                    'S 200 p1_results 00000000 -> 00000001\nS 700 p1_phase 00000003 -> 00000001\n',
                    'S 200 p1_results 00000000 -> 00000001\nS 300 p1_results 00000001 -> 00000000\nS 700 p1_phase 00000001 -> 00000002\n']:
            (self.root / 'time').unlink(missing_ok=True)
            with self.subTest(log=log):
                self.assertEqual(self.wait(log).returncode, 9)


if __name__ == '__main__':
    unittest.main()
