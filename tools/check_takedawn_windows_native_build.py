"""Maintainer-only standalone Windows SDK pipeline gate with synthetic Rust.

Builds and runs native Windows tools directly. Python only orchestrates this
asset-free CI check; it is not part of the player build or runtime.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import subprocess
import tarfile
import tempfile
import urllib.request
import zipfile


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(part)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    if os.name != "nt":
        parser.error("Run this standalone SDK gate on actual Windows")
    project = Path(__file__).resolve().parents[1]
    pins = json.loads(
        (project / "engines/takedawn/windows-dependencies.json").read_text()
    )
    work = Path(tempfile.mkdtemp(prefix="gamepkg-windows-native-build."))
    print("Private evidence directory:", work, flush=True)
    rust = work / "rust"
    llvm = work / "llvm"
    rust.mkdir()
    llvm.mkdir()

    def fetch(record):
        path = work / record["url"].rsplit("/", 1)[-1]
        with (
            urllib.request.urlopen(record["url"], timeout=120) as response,
            path.open("wb") as output,
        ):
            shutil.copyfileobj(response, output)
        if digest(path) != record["sha256"]:
            raise RuntimeError("Official SDK component digest mismatch: " + path.name)
        return path

    for component, record in pins["rust"]["components"].items():
        package = fetch(record)
        payload = (
            "rust-std-" + pins["rust"]["target"]
            if component == "rust-std"
            else component
        )
        copied = 0
        with tarfile.open(package, "r:xz") as archive:
            for item in archive:
                name = PurePosixPath(item.name)
                if name.is_absolute() or ".." in name.parts:
                    raise RuntimeError("Unsafe SDK archive path")
                if len(name.parts) < 3 or name.parts[1] != payload:
                    continue
                path = rust.joinpath(*name.parts[2:])
                if item.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                    continue
                if not item.isfile():
                    raise RuntimeError("Unexpected SDK link or special file")
                path.parent.mkdir(parents=True, exist_ok=True)
                data = archive.extractfile(item).read()
                if path.exists() and path.read_bytes() != data:
                    raise RuntimeError("Conflicting SDK component file")
                path.write_bytes(data)
                copied += 1
        if not copied:
            raise RuntimeError("Empty SDK component")
        print("Verified standalone Rust component:", component, flush=True)
    package = fetch(pins["llvm_mingw"]["windows_x86_64"])
    with zipfile.ZipFile(package) as archive:
        for item in archive.infolist():
            name = PurePosixPath(item.filename)
            if (
                name.is_absolute()
                or ".." in name.parts
                or stat.S_ISLNK(item.external_attr >> 16)
            ):
                raise RuntimeError("Unsafe LLVM SDK member")
            if len(name.parts) < 2:
                continue
            path = llvm.joinpath(*name.parts[1:])
            if item.is_dir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(archive.read(item))
    rustc = rust / "bin/rustc.exe"
    # Keep the compilation PATH to the verified SDK plus Windows system tools;
    # no rustup/MSVC/WSL/Python executable participates in native compilation.
    env = dict(
        os.environ,
        PATH=os.pathsep.join(
            [
                str(rust / "bin"),
                str(llvm / "bin"),
                str(Path(os.environ["SystemRoot"]) / "System32"),
            ]
        ),
    )
    tools = project / "engines/takedawn/rust-port"
    wrapper = work / "cache.exe"
    builder = work / "build-units.exe"
    for source, output in [
        (tools / "rustc-cache.rs", wrapper),
        (tools / "build-windows-units.rs", builder),
    ]:
        subprocess.run(
            [
                str(rustc),
                "--edition=2021",
                "-Dwarnings",
                "-O",
                str(source),
                "-o",
                str(output),
            ],
            env=env,
            check=True,
            timeout=120,
        )
    prepared = work / "private prepared sources"
    (prepared / "src").mkdir(parents=True)
    (prepared / "units.txt").write_text("fixture_a\nfixture_b\n")
    (prepared / "exports.txt").write_text("gp_build_a\ngp_build_b\n")
    (prepared / "src/lib.rs").write_text("// Prepared native fixture\n")
    (prepared / "src/fixture_a.rs").write_text(
        '#[no_mangle] pub extern "C" fn gp_build_a() -> u64 { 7 }\n'
    )
    (prepared / "src/fixture_b.rs").write_text(
        'unsafe extern "C" { fn gp_build_a() -> u64; }\n#[no_mangle] pub extern "C" fn gp_build_b() -> u64 { let v=Vec::<u64>::with_capacity(unsafe { gp_build_a() } as usize); std::hint::black_box(v).capacity() as u64 + 4 }\n'
    )
    output = work / "native output"
    command = [
        str(builder),
        "--prepared",
        str(prepared),
        "--rustc",
        str(rustc),
        "--ar",
        str(llvm / "bin/llvm-ar.exe"),
        "--nm",
        str(llvm / "bin/llvm-nm.exe"),
        "--cache-wrapper",
        str(wrapper),
        "--output",
        str(output),
        "--cache",
        str(work / "unit cache"),
        "--cpu",
        "native",
    ]
    first = subprocess.run(command, env=env, capture_output=True, timeout=180)
    (work / "first.stdout.log").write_bytes(first.stdout)
    (work / "first.stderr.log").write_bytes(first.stderr)
    if first.returncode:
        raise RuntimeError(first.stderr.decode(errors="replace"))
    record = json.loads((output / "qualification.json").read_text())
    assert record["units"] == 2 and record["retained_exports"] == 2
    assert record["archive_sha256"] == digest(
        output / "libgamepkg_takedawn_generated_full_windows.a"
    )
    again = subprocess.run(command, env=env, capture_output=True, timeout=180)
    (work / "again.stdout.log").write_bytes(again.stdout)
    (work / "again.stderr.log").write_bytes(again.stderr)
    assert (
        again.returncode == 0 and again.stderr.count(b"reused prepared Rust unit") == 2
    )
    main = work / "main.c"
    main.write_text(
        '#include <stdint.h>\n#include <stdio.h>\nextern uint64_t gp_build_b(void);\nint main(void) { uint64_t n=gp_build_b(); printf("Native archive value: %llu\\n", (unsigned long long)n); return n==11 ? 0 : 1; }\n'
    )
    exe = work / "native-fixture.exe"
    subprocess.run(
        [
            str(llvm / "bin/x86_64-w64-mingw32-clang.exe"),
            str(main),
            str(output / "libgamepkg_takedawn_generated_full_windows.a"),
            "-static",
            "-lws2_32",
            "-luserenv",
            "-lbcrypt",
            "-lntdll",
            "-o",
            str(exe),
        ],
        env=env,
        check=True,
        timeout=120,
    )
    result = subprocess.run([str(exe)], env=env, capture_output=True, timeout=20)
    (work / "native.stdout.log").write_bytes(result.stdout)
    (work / "native.stderr.log").write_bytes(result.stderr)
    assert result.returncode == 0 and b"Native archive value: 11" in result.stdout
    print(
        "PASS: standalone native Windows Rust/LLVM compile, both checkpoints reused, full archive exports, native C/Rust heap ABI and execution; no WSL or Python build dependency",
        flush=True,
    )


if __name__ == "__main__":
    main()
