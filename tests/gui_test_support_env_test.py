"""Keep GUI tools on the caller's private display and worker limits."""
import tempfile
import unittest
from unittest.mock import patch

from gui_test_support import click_text, game_window
from types import SimpleNamespace
import re


class GuiToolEnvironmentTest(unittest.TestCase):
    def test_ocr_and_display_tools_receive_the_callers_environment(self):
        env = {'DISPLAY': ':97', 'OMP_THREAD_LIMIT': '1',
               'XDG_CACHE_HOME': '/private-test-cache'}
        ocr_calls = []

        def run(arguments, **kwargs):
            self.assertEqual(kwargs.get('env'), env, arguments)

        def recognize(arguments, **kwargs):
            self.assertEqual(kwargs.get('env'), env, arguments)
            ocr_calls.append(arguments)
            return 'text\tleft\ttop\twidth\theight\nAction\t30\t60\t120\t30\n'

        for click in (False, True):
            with self.subTest(click=click), tempfile.TemporaryDirectory() as directory:
                with patch('gui_test_support.subprocess.run', side_effect=run), patch(
                        'gui_test_support.subprocess.check_output', side_effect=recognize):
                    click_text('123', env, 'Action', directory, click=click)
        self.assertEqual(len(ocr_calls), 2)
        self.assertTrue(all(call[0] == 'tesseract' for call in ocr_calls))

    def test_versioned_app_title_and_exact_game_title(self):
        for title, accepted, rejected in [
            ('GamePKG', 'GamePKG 0.1.24', ['GamePKG', 'GamePKG settings', 'GamePKG 0x1x24']),
            ('org.example.game', 'org.example.game', ['orgXexampleXgame', 'org.example.game settings']),
        ]:
            def run(arguments, **kwargs):
                pattern = arguments[-1]
                self.assertIsNotNone(re.fullmatch(pattern, accepted))
                for wrong in rejected:
                    self.assertIsNone(re.fullmatch(pattern, wrong))
                return SimpleNamespace(returncode=0, stdout='123\n')
            with self.subTest(title=title), patch('gui_test_support.subprocess.run', side_effect=run), patch('gui_test_support.subprocess.check_output', return_value='456'):
                self.assertEqual(game_window({'DISPLAY': ':97'}, title), ('123', 456))


if __name__ == '__main__':
    unittest.main()
