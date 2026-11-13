#!/usr/bin/env python3
"""Test production display transitions, and the native decoder on Windows.

Python and Rust are maintainer test tools, not additional player requirements.
Real Vulkan presentation and a physical lid cycle need separate device tests.
"""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile


def checked_output(arguments, expected):
    result = subprocess.run(arguments, capture_output=True, text=True, timeout=1800)
    print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="")
    result.check_returncode()
    if not re.search(rf"test result: ok\. {expected} passed; 0 failed;", result.stdout):
        raise RuntimeError(f"Expected {expected} production tests to execute successfully")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=["i686-pc-windows-msvc", "x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"])
    parser.add_argument("--vulkan", action="store_true", help="Also exercise a native Vulkan window; requires an active desktop and available panel")
    options = parser.parse_args()
    if (options.target or options.vulkan) and os.name != "nt":
        parser.error("Run a Windows target on actual Windows")
    project = Path(__file__).resolve().parents[1]
    source = project / "vendor/winit/src/platform_impl/windows/display_restore_state.rs"
    with tempfile.TemporaryDirectory(prefix="gamepkg-display-restore-") as directory:
        root = Path(directory)
        harness = root / "tests.rs"
        harness.write_text(
            f"#[path = {json.dumps(str(source), ensure_ascii=False)}]\n"
            "mod display_restore_state;\n",
            encoding="utf-8",
        )
        binary = root / ("tests.exe" if os.name == "nt" else "tests")
        arguments = ["rustc", "--edition=2021", "--test", str(harness), "-o", str(binary)]
        if options.target:
            arguments += ["--target", options.target]
        subprocess.run(arguments, check=True)
        checked_output([str(binary)], 7)
        if os.name == "nt":
            fixture = project / "tests/fixtures/windows-display-restore/Cargo.toml"
            arguments = ["cargo", "test", "--locked", "--manifest-path", str(fixture), "--target-dir", str(root / "target")]
            if options.target:
                arguments += ["--target", options.target]
            checked_output(arguments, 10)
            if options.vulkan:
                environment = dict(os.environ, GAMEPKG_RENDER_PROBE_LOG=str(root / "vulkan-cycle.log"))
                arguments = ["cargo", "build", "--locked", "--manifest-path", str(fixture), "--target-dir", str(root / "target"), "--features", "vulkan", "--bin", "vulkan-cycle"]
                if options.target:
                    arguments += ["--target", options.target]
                subprocess.run(arguments, env=environment, check=True, timeout=1800)
                executable = root / "target"
                if options.target:
                    executable /= options.target
                executable /= "debug/vulkan-cycle.exe"
                result = subprocess.run([str(executable)], env=environment, timeout=60)
                evidence = (root / "vulkan-cycle.log").read_text(encoding="utf-8")
                print(evidence, end="")
                result.check_returncode()
                if "COMPLETE steps14" not in evidence or not evidence.rstrip().endswith("PASS"):
                    raise RuntimeError("Native Vulkan notification cycle did not complete")


if __name__ == "__main__":
    main()
