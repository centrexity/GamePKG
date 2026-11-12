"""Maintainer-only native Rust checkpoint gate; no game files or Python player dependency."""

import argparse
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    source = project / "engines/takedawn/rust-port/rustc-cache.rs"
    work = Path(tempfile.mkdtemp(prefix="gamepkg-native-rust-cache."))
    print("Private evidence directory:", work, flush=True)
    windows = "-windows-" in args.target
    if windows != (os.name == "nt"):
        parser.error("Run this gate on the native operating system of its target")
    binary = work / ("cache-tests.exe" if windows else "cache-tests")
    subprocess.run(
        [
            "rustc",
            "--edition=2021",
            "--target",
            args.target,
            "--test",
            str(source),
            "-o",
            str(binary),
        ],
        check=True,
    )
    env = dict(os.environ, GAMEPKG_CACHE_TEST_TARGET=args.target)
    result = subprocess.run(
        [str(binary), "--nocapture"], capture_output=True, timeout=90, env=env
    )
    (work / "stdout.log").write_bytes(result.stdout)
    (work / "stderr.log").write_bytes(result.stderr)
    print(result.stdout.decode(errors="replace"), flush=True)
    if result.returncode:
        print(result.stderr.decode(errors="replace"), flush=True)
    result.check_returncode()
    if windows:
        if not args.target.endswith("-msvc"):
            parser.error("The native Windows fixture uses the configured MSVC compiler")
        c = work / "native-bigobj.c"
        c.write_text("int gamepkg_native_marker(void) { return 7; }\n")
        obj = work / "native-bigobj.obj"
        subprocess.run(
            [
                "cl",
                "/nologo",
                "/std:c11",
                "/W4",
                "/WX",
                "/bigobj",
                "/c",
                str(c),
                "/Fo" + str(obj),
            ],
            cwd=work,
            check=True,
        )
        subprocess.run(
            [
                str(binary),
                "tests::native_bigobj_is_a_relocatable_object_not_a_short_import",
                "--exact",
                "--ignored",
                "--nocapture",
            ],
            check=True,
            timeout=30,
            env=dict(env, GAMEPKG_CACHE_TEST_BIGOBJ=str(obj)),
        )
    print(
        "PASS: native hashing, actual compiler objects, relocation between private source trees, corruption recovery and invalid-source rejection",
        flush=True,
    )


if __name__ == "__main__":
    main()
