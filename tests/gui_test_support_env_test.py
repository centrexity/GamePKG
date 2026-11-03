"""Keep GUI tools on the caller's private display and worker limits."""
import tempfile
import subprocess
import sys
import unittest
from unittest.mock import patch

from gui_test_support import click_text, stop_process


class GuiToolEnvironmentTest(unittest.TestCase):
    def test_shutdown_drains_full_diagnostic_pipe(self):
        script = '''
import os, signal, time
def stop(*args):
    os.write(2, b'x' * 262144)
    raise SystemExit(0)
signal.signal(signal.SIGTERM, stop)
print('ready', flush=True)
while True: time.sleep(1)
'''
        with subprocess.Popen([sys.executable, '-c', script], stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE) as process:
            self.assertEqual(process.stdout.readline(), b'ready\n')
            _, diagnostics = stop_process(process, timeout=2)
            self.assertEqual(process.returncode, 0)
            self.assertEqual(diagnostics, b'x' * 262144)

    def test_shutdown_kills_and_reaps_unresponsive_process(self):
        script = '''
import signal, time
signal.signal(signal.SIGTERM, signal.SIG_IGN)
print('ready', flush=True)
while True: time.sleep(1)
'''
        with subprocess.Popen([sys.executable, '-c', script], stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE) as process:
            self.assertEqual(process.stdout.readline(), b'ready\n')
            stop_process(process, timeout=.1)
            self.assertEqual(process.returncode, -9)

    def test_shutdown_reaps_already_exited_process(self):
        with subprocess.Popen([sys.executable, '-c', 'print("finished")'],
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE) as process:
            process.wait(timeout=2)
            self.assertEqual(stop_process(process), (b'finished\n', b''))
            self.assertEqual(process.returncode, 0)

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
