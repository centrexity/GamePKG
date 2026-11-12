"""Maintainer tests: compiled Android suites must execute, never merely build."""
from pathlib import Path
import json
import os
import re
import subprocess
import tempfile
import unittest

PROJECT = Path(__file__).resolve().parents[1]


class AndroidRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="android runner control ")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        workflow = (PROJECT / ".github/workflows/portability-policy.yml").read_text()
        command, = [line for line in workflow.splitlines()
                    if "cargo test " in line and "--target i686-linux-android " in line]
        self.compiled = set(re.findall(r"--test ([a-z_]+)", command))
        self.assertIn("render_resources", self.compiled)
        self.assertIn("game_arkanoo", self.compiled)
        binaries = self.root / "target/i686-linux-android/debug/deps"
        binaries.mkdir(parents=True)
        for name in self.compiled:
            binary = binaries / (name + "-control")
            binary.write_text("#!/bin/sh\nexit 0\n")
            binary.chmod(0o755)
        self.publication = self.root / "library-control"
        self.publication.write_text("#!/bin/sh\nexit 0\n")
        self.publication.chmod(0o755)
        self.log = self.root / "adb.jsonl"
        adb = self.root / "fake-adb"
        adb.write_text('''#!/usr/bin/python3
import json,os,sys
args=sys.argv[1:]
assert args[:2]==['-s','owned-control-device'],args
with open(os.environ['CONTROL_ADB_LOG'],'a') as output:
 output.write(json.dumps(args)+'\\n')
assert args[2] in ['push','shell'],args
if args[2]=='push':
 assert os.path.isfile(args[3]),args
elif '--test-threads=1' in args[3] and os.environ.get('CONTROL_FAIL_SUITE'):
 if '/'+os.environ['CONTROL_FAIL_SUITE']+'-control ' in args[3]:sys.exit(41)
''')
        adb.chmod(0o755)
        self.environment = dict(os.environ, ADB=str(adb), CONTROL_ADB_LOG=str(self.log))
        self.environment.pop("CONTROL_FAIL_SUITE", None)

    def run_runner(self):
        result = subprocess.run(
            ["bash", str(PROJECT / "tools/test_android_runtime.sh"),
             "owned-control-device", "i686-linux-android", str(self.publication)],
            cwd=self.root, env=self.environment, capture_output=True, text=True, timeout=10,
        )
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        suites = []
        publication = []
        for args in calls:
            if args[2] == "shell":
                match = re.search(r"/([a-z_]+)-control --test-threads=1", args[3])
                if match:
                    suites.append(match[1])
                if "pack::publication_tests::" in args[3]:
                    publication.append(args)
        return result, suites, publication

    def test_every_compiled_suite_executes_before_publication(self):
        result, suites, publication = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertCountEqual(suites, self.compiled)
        self.assertEqual(len(publication), 1)

    def test_suite_failure_stops_before_publication(self):
        self.environment["CONTROL_FAIL_SUITE"] = "render_resources"
        result, suites, publication = self.run_runner()
        self.assertEqual(result.returncode, 41, result.stderr)
        self.assertEqual(suites[-1], "render_resources")
        self.assertEqual(publication, [])

    def test_missing_compiled_suite_cannot_report_success(self):
        (self.root / "target/i686-linux-android/debug/deps/game_arkanoo-control").unlink()
        result, suites, publication = self.run_runner()
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("game_arkanoo", suites)
        self.assertEqual(publication, [])


if __name__ == "__main__":
    unittest.main()
