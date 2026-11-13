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
        self.manifest = PROJECT / "Cargo.toml"
        self.artifacts = self.root / "android-tests.json"
        self.records = [
            {"reason": "compiler-artifact", "profile": {"test": True},
             "manifest_path": str(self.manifest),
             "target": {"name": name, "kind": ["test"]},
             "executable": str(binaries / (name + "-control"))}
            for name in sorted(self.compiled)
        ]
        self.records.append(
            {"reason": "compiler-artifact", "profile": {"test": True},
             "manifest_path": str(self.manifest),
             "target": {"name": "control", "kind": ["lib"]},
             "executable": str(self.publication)}
        )
        self.write_artifacts()
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

    def write_artifacts(self):
        self.artifacts.write_text("\n".join(json.dumps(record) for record in self.records) + "\n")

    def run_runner(self):
        result = subprocess.run(
            ["bash", str(PROJECT / "tools/test_android_runtime.sh"),
             "owned-control-device", "i686-linux-android", str(self.artifacts)],
            cwd=self.root, env=self.environment, capture_output=True, text=True, timeout=10,
        )
        calls = [json.loads(line) for line in self.log.read_text().splitlines()] if self.log.exists() else []
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

    def test_custom_cargo_artifact_directory_is_not_assumed_to_be_target(self):
        custom = self.root / "Cargo artifacts elsewhere with spaces"
        custom.mkdir()
        for record in self.records:
            source = Path(record["executable"])
            destination = custom / source.name
            source.rename(destination)
            record["executable"] = str(destination)
        self.write_artifacts()
        result, suites, publication = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertCountEqual(suites, self.compiled)
        self.assertEqual(len(publication), 1)

    def test_stale_executable_on_disk_is_not_uploaded_or_run(self):
        stale = self.root / "target/i686-linux-android/debug/deps/components-stale"
        stale.write_text("stale test binary")
        stale.chmod(0o755)
        result, suites, publication = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertCountEqual(suites, self.compiled)
        self.assertNotIn("components-stale", self.log.read_text())
        self.assertEqual(len(publication), 1)

    def test_duplicate_reported_suite_is_rejected_before_device_changes(self):
        self.records.append(self.records[0].copy())
        self.write_artifacts()
        result, suites, publication = self.run_runner()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(suites, [])
        self.assertEqual(publication, [])
        self.assertFalse(self.log.exists())

    def test_foreign_package_test_artifacts_are_not_selected(self):
        foreign = self.records[0].copy()
        foreign["manifest_path"] = str(self.root / "foreign/Cargo.toml")
        self.records.append(foreign)
        self.write_artifacts()
        result, suites, publication = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertCountEqual(suites, self.compiled)
        self.assertEqual(len(publication), 1)

    def test_local_executable_bit_is_not_required_for_android_upload(self):
        for record in self.records:
            Path(record["executable"]).chmod(0o644)
        result, suites, publication = self.run_runner()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertCountEqual(suites, self.compiled)
        calls = [json.loads(line) for line in self.log.read_text().splitlines()]
        permissions = [args for args in calls if args[2] == "shell" and args[3].startswith("chmod 700 ")]
        self.assertEqual(len(permissions), 9)
        self.assertEqual(len(publication), 1)

    def test_unsafe_remote_filename_is_rejected_before_device_changes(self):
        source = Path(self.records[0]["executable"])
        unsafe = source.with_name(source.name + ";unexpected")
        source.rename(unsafe)
        self.records[0]["executable"] = str(unsafe)
        self.write_artifacts()
        result, suites, publication = self.run_runner()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(suites, [])
        self.assertEqual(publication, [])
        self.assertFalse(self.log.exists())

    def test_malformed_metadata_is_rejected_before_device_changes(self):
        with self.artifacts.open("a") as output:
            output.write("{invalid json}\n")
        result, suites, publication = self.run_runner()
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(suites, [])
        self.assertEqual(publication, [])
        self.assertFalse(self.log.exists())


if __name__ == "__main__":
    unittest.main()
