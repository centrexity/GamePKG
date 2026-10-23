"""Maintainer-only native upstream restart/teardown staging and ownership gate."""

import argparse
import os
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument(
        "--target",
        required=True,
        choices=[
            "x86_64-pc-windows-msvc",
            "i686-pc-windows-msvc",
            "x86_64-pc-windows-gnu",
            "i686-pc-windows-gnu",
        ],
    )
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    source = args.source.resolve()
    before = (source / "src/platform/recomp_restart.c").read_bytes()
    assert (
        subprocess.check_output(
            ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
        ).strip()
        == "5ae8ad90d984425e834dee759592c15592679880"
    )
    canonical = subprocess.check_output(
        ["git", "-C", str(source), "show", "HEAD:src/platform/recomp_restart.c"]
    )
    work = Path(tempfile.mkdtemp(prefix="gamepkg-windows-restart."))
    print("Private evidence directory:", work, flush=True)
    original = work / "original.c"
    original.write_bytes(canonical)
    bridge = project / "engines/takedawn/media-bridge"
    staged = work / "recomp_restart.c"

    def stage(src, dst, expected):
        result = subprocess.run(
            [
                "cmake",
                "-DINPUT=" + str(src),
                "-DOUTPUT=" + str(dst),
                "-P",
                str(bridge / "adapt_windows_restart.cmake"),
            ],
            capture_output=True,
        )
        if (result.returncode == 0) != expected:
            raise RuntimeError(
                result.stdout.decode(errors="replace")
                + result.stderr.decode(errors="replace")
            )

    stage(original, staged, True)
    text = staged.read_text()
    include = (
        '#if defined(_WIN32)\n#include "'
        + (bridge / "windows_restart.h").as_posix()
        + '"\n#endif\n'
    )
    assert (
        text.count(include) == 1
        and text.count("    if (gpw_restart_at_exit()) return;\n") == 1
    )
    assert (
        text.replace(include, "")
        .replace("    if (gpw_restart_at_exit()) return;\n", "")
        .encode()
        == canonical
    )
    stage(original, original, False)
    changed = work / "changed.c"
    changed.write_bytes(canonical + b"\n/* different source */\n")
    stage(changed, work / "rejected.c", False)
    assert not (work / "rejected.c").exists()
    assert (
        original.read_bytes() == canonical
        and (source / "src/platform/recomp_restart.c").read_bytes() == before
    )
    exe = work / "restart.exe"
    if args.target.endswith("-msvc"):
        command = [
            "cl",
            "/nologo",
            "/std:c11",
            "/O2",
            "/W4",
            "/WX",
            "/UNDEBUG",
            "/D_CRT_SECURE_NO_WARNINGS",
            "/I" + str(source / "src/platform"),
            str(staged),
            str(bridge / "tests/windows_restart.c"),
            "/Fe" + str(exe),
        ]
    else:
        cc = args.target.replace("pc-windows-gnu", "w64-mingw32") + "-gcc"
        command = [
            cc,
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-UNDEBUG",
            "-I",
            str(source / "src/platform"),
            str(staged),
            str(bridge / "tests/windows_restart.c"),
            "-o",
            str(exe),
        ]
    subprocess.run(command, cwd=work, check=True)
    if os.name != "nt":
        print(
            "PASS: pinned staging/restoration and Windows fixture compilation; native execution still required"
        )
        return
    import ctypes
    from ctypes import wintypes

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateEventW.argtypes = [
        ctypes.c_void_p,
        wintypes.BOOL,
        wintypes.BOOL,
        wintypes.LPCWSTR,
    ]
    kernel.CreateEventW.restype = wintypes.HANDLE
    kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
    kernel.WaitForSingleObject.restype = wintypes.DWORD
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    for mode in ["request", "cancel", "quit"]:
        folder = work / mode
        folder.mkdir()
        event = kernel.CreateEventW(None, True, False, None)
        if not event:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            os.set_handle_inheritable(event, True)
            startup = subprocess.STARTUPINFO()
            startup.lpAttributeList = {"handle_list": [event]}
            result = subprocess.run(
                [str(exe), mode, str(folder / "marker.txt")],
                cwd=folder,
                env=dict(os.environ, GAMEPKG_TAKEDAWN_RESTART_HANDLE=str(event)),
                startupinfo=startup,
                close_fds=True,
                capture_output=True,
                timeout=20,
            )
            (folder / "stdout.log").write_bytes(result.stdout)
            (folder / "stderr.log").write_bytes(result.stderr)
            assert result.returncode == 0, (mode, result.returncode, result.stderr)
            assert (
                folder / "marker.txt"
            ).read_text() == "teardown complete\nfinal cleanup complete\n"
            assert kernel.WaitForSingleObject(event, 0) == (
                0 if mode == "request" else 258
            )
            assert (
                "managed restart requested; normal exit pending"
                in result.stderr.decode()
            ) == (mode == "request")
        finally:
            kernel.CloseHandle(event)
    subprocess.run(
        [
            "cargo",
            "test",
            "--locked",
            "--manifest-path",
            str(project / "engines/takedawn/windows-runner/Cargo.toml"),
            "--target",
            args.target,
            "tests::actual_upstream_restart_teardown",
            "--",
            "--ignored",
            "--exact",
            "--nocapture",
        ],
        cwd=project,
        check=True,
        env=dict(os.environ, GAMEPKG_RESTART_FIXTURE=str(exe)),
    )
    print(
        "PASS: original teardown/request/cancel/quit and Rust-owned restart with retained ISO"
    )


if __name__ == "__main__":
    main()
