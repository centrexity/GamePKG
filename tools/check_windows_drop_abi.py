"""Maintainer-only native Windows SDK check of production file-drop callbacks.

Runs on actual Windows with an MSVC development environment matching --target.
No window, game media, audio or user library is used. Cargo dependencies are
locked; Python and the C compiler are test tools, not player requirements.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "--target",
    required=True,
    choices=["i686-pc-windows-msvc", "x86_64-pc-windows-msvc"],
)
options = parser.parse_args()
if os.name != "nt":
    parser.error("Run this SDK ABI check on actual Windows")
project = Path(__file__).resolve().parents[1]
fixture = project / "tests/fixtures/windows-drop-abi"
work = Path(tempfile.mkdtemp(prefix="gamepkg-drop-abi."))
print("Private native SDK ABI evidence:", work, flush=True)


def run(label, arguments, environment=None):
    result = subprocess.run(
        arguments, cwd=work, env=environment, capture_output=True, timeout=180
    )
    (work / (label + ".stdout.log")).write_bytes(result.stdout)
    (work / (label + ".stderr.log")).write_bytes(result.stderr)
    assert result.returncode == 0, (
        label,
        result.stdout.decode(errors="replace"),
        result.stderr.decode(errors="replace"),
    )
    return result.stdout


environment = dict(os.environ, CARGO_TARGET_DIR=str(work / "target"))
run(
    "production-vtable",
    [
        "cargo",
        "build",
        "--locked",
        "--manifest-path",
        str(fixture / "Cargo.toml"),
        "--target",
        options.target,
    ],
    environment,
)
dll = work / "Drop callbacks with spaces.dll"
run(
    "native-sdk",
    [
        "cl.exe",
        "/nologo",
        "/LD",
        "/TC",
        "/W3",
        "/WX",
        str(fixture / "callbacks.c"),
        "/Fo" + str(work / "callbacks.obj"),
        "/Fe" + str(dll),
    ],
)
executable = (
    work / "target" / options.target / "debug/gamepkg-windows-drop-abi-check.exe"
)
output = run("callback-boundary", [str(executable), str(dll)])
assert b"PASS: production Windows drop vtable matches native SDK callbacks" in output
(work / "result.json").write_text(
    json.dumps(
        {
            "target": options.target,
            "actual_windows_sdk": True,
            "production_vtable": True,
            "production_vtable_sha256": hashlib.sha256(
                (
                    project / "vendor/winit/src/platform_impl/windows/definitions.rs"
                ).read_bytes()
            ).hexdigest(),
            "callbacks": ["DragEnter", "DragOver", "DragLeave", "Drop"],
            "exit": 0,
        },
        indent=2,
    )
    + "\n"
)
print(output.decode().strip(), flush=True)
