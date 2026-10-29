"""Adversarial startup/cleanup tests for our owned virtual display helper."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from gui_test_support import start_display

class DisplayStartup(unittest.TestCase):
    def fixture(self, root, body):
        server=root/'Xvfb'
        server.write_text('#!/usr/bin/python3\nimport os,sys,time,signal\n'+body)
        server.chmod(0o755)
        client=root/'xdotool';client.write_text('#!/bin/sh\nprintf "1024 900\\n"\n');client.chmod(0o755)
        return dict(os.environ,PATH=str(root)+os.pathsep+os.environ['PATH'])

    def test_large_stderr_cannot_block_the_ready_handshake(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)
            env=self.fixture(root,"os.write(2,b'x'*262144+b'final startup marker\\n')\nfd=int(sys.argv[sys.argv.index('-displayfd')+1]);os.write(fd,b'81\\n')\nwhile True: signal.pause()\n")
            process=start_display(env,startup_timeout=2)
            try:
                self.assertEqual(env['DISPLAY'],':81')
            finally:
                process.terminate();process.wait(timeout=5)
                process._gamepkg_display_collector.join(timeout=1)
                process.stderr.close()
            data=bytes(process._gamepkg_display_diagnostics)
            self.assertLessEqual(len(data),65536)
            self.assertTrue(data.endswith(b'final startup marker\n'))

    def test_early_exit_retains_the_actual_server_error(self):
        with tempfile.TemporaryDirectory() as temporary:
            env=self.fixture(Path(temporary),"os.write(2,b'fixture bad screen size\\n');sys.exit(7)\n")
            with self.assertRaisesRegex(RuntimeError,'fixture bad screen size') as failure:
                start_display(env,startup_timeout=2)
            self.assertIn('exit=7',str(failure.exception))

    def test_unready_process_is_stopped_and_diagnostics_survive(self):
        with tempfile.TemporaryDirectory() as temporary:
            env=self.fixture(Path(temporary),"os.write(2,b'fixture waiting for display\\n')\nwhile True: signal.pause()\n")
            owned=[];original=subprocess.Popen
            def launch(*args,**kwargs):
                process=original(*args,**kwargs);owned.append(process);return process
            with patch.object(subprocess,'Popen',side_effect=launch):
                with self.assertRaisesRegex(RuntimeError,'fixture waiting for display'):
                    start_display(env,startup_timeout=.2)
            self.assertEqual(len(owned),1)
            self.assertIsNotNone(owned[0].poll())
            owned[0].stderr.close()

if __name__=='__main__':
    unittest.main()
