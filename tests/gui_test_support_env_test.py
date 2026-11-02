"""Keep GUI tools on the caller's private display and worker limits."""
import tempfile
import unittest
from unittest.mock import patch

from gui_test_support import click_text


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


if __name__ == '__main__':
    unittest.main()
