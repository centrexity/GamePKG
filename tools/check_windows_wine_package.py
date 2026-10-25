"""Maintainer-only silent Wine check of a packaged Windows CLI and GUI.

Uses a disposable prefix/library, no network and no owned game media. Wine and
Python are test tools, not player requirements. --gui needs an X11 display.
--drop additionally uses a MinGW-built OLE source and moves the pointer only
over the owned test window to import a catalog without downloading anything.
Actual Windows hardware and AppContainer gameplay remain separate checks.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import tempfile
import time
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--gui", action="store_true")
    parser.add_argument(
        "--drop",
        action="store_true",
        help="Exercise actual Windows OLE catalog drop (requires --gui)",
    )
    options = parser.parse_args()
    if options.drop and not options.gui:
        parser.error("--drop requires --gui")
    required = ["bwrap", "wine", "wineboot", "wineserver", "timeout"]
    if options.drop:
        required.append("x86_64-w64-mingw32-gcc")
    if options.gui:
        required += ["xdotool", "xprop", "xwd", "ffmpeg"]
        if (
            not os.environ.get("DISPLAY")
            or not Path(os.environ.get("XAUTHORITY", "")).is_file()
        ):
            parser.error("--gui requires DISPLAY and a readable XAUTHORITY")
    for tool in required:
        if not shutil.which(tool):
            parser.error("Missing maintainer tool: " + tool)
    archive = options.archive.resolve(strict=True)
    expected = archive.with_suffix(archive.suffix + ".sha256").read_text().split()[0]
    with archive.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if digest != expected:
        parser.error("Windows archive checksum mismatch")
    project = Path(__file__).resolve().parents[1]
    fixture = project / "examples/good/collect-stars"
    work = Path(tempfile.mkdtemp(prefix="gamepkg-wine-package."))
    print("Private Wine package evidence:", work, flush=True)
    for folder in ["bin", "home"]:
        (work / folder).mkdir()
    if options.drop:
        compiled = subprocess.run(
            [
                "x86_64-w64-mingw32-gcc",
                "-municode",
                "-Os",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wno-unused-parameter",
                str(project / "tests/fixtures/windows-ole-drop-source.c"),
                "-o",
                str(work / "bin/drop-source.exe"),
                "-lole32",
                "-luuid",
            ],
            capture_output=True,
            timeout=60,
        )
        (work / "drop-source-build.log").write_bytes(compiled.stdout + compiled.stderr)
        assert compiled.returncode == 0, (
            "Native OLE fixture compilation failed",
            str(work),
        )
        (work / "dropped.gamepkglist").write_text(
            'format_version=1\n\n[[games]]\nid="com.gamepkg.wine-drop-catalog"\n'
            'title="Wine Drop Catalog Fixture"\n'
            'download_url="https://example.invalid/wine-drop.gamepkg"\nrequires_rom=false\n'
        )
    gui_name = "gamepkg-wine-" + work.name.rsplit(".", 1)[-1] + ".exe"
    with zipfile.ZipFile(archive) as bundle:
        root = archive.stem + "/"
        metadata = json.loads(bundle.read(root + "BUILD.json"))
        if metadata["platform"] != "windows-x86_64":
            parser.error("This Wine gate requires a Windows x86-64 package")
        for name in ["gamepkg.exe", "gamepkg-gui.exe"]:
            data = bundle.read(root + "bin/" + name)
            assert (
                hashlib.sha256(data).hexdigest() == metadata["executables_sha256"][name]
            )
            (
                work / "bin" / (gui_name if name == "gamepkg-gui.exe" else name)
            ).write_bytes(data)
    (work / "settings.toml").write_text(
        "automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\n"
    )
    sandbox = [
        "timeout",
        "--signal=TERM",
        "--kill-after=5s",
        "150s",
        "bwrap",
        "--die-with-parent",
        "--unshare-net",
        "--unshare-pid",
        "--clearenv",
    ]
    for path in ["/usr", "/lib", "/lib64", "/etc", "/sys"]:
        sandbox += ["--ro-bind", path, path]
    sandbox += [
        "--symlink",
        "usr/bin",
        "/bin",
        "--dev-bind",
        "/dev",
        "/dev",
        "--proc",
        "/proc",
        "--tmpfs",
        "/tmp",
        "--bind",
        str(work),
        "/state",
        "--ro-bind",
        str(work / "bin"),
        "/app",
        "--ro-bind",
        str(fixture),
        "/fixture",
        "--chdir",
        "/state",
    ]
    environment = {
        "PATH": "/usr/bin:/bin",
        "HOME": "/state/home",
        "LANG": "C.UTF-8",
        "WINEARCH": "win64",
        "WINEPREFIX": "/state/prefix",
        "WINEDEBUG": "-all",
        "WINEDLLOVERRIDES": "mscoree,mshtml=",
        "SDL_AUDIODRIVER": "dummy",
        "GAMEPKG_HOME": "Z:\\state\\library",
        "GAMEPKG_CONFIG": "Z:\\state\\settings.toml",
        "GAMEPKG_AUDIO_DECODER_WASM": "Z:\\missing.wasm",
        "GAMEPKG_BUNDLED_AUDIO_DECODER": "Z:\\missing.wasm",
    }
    if options.gui:
        sandbox += [
            "--ro-bind",
            "/tmp/.X11-unix",
            "/tmp/.X11-unix",
            "--ro-bind",
            os.environ["XAUTHORITY"],
            "/xauth",
        ]
        environment.update(DISPLAY=os.environ["DISPLAY"], XAUTHORITY="/xauth")
    for key, value in environment.items():
        sandbox += ["--setenv", key, value]

    records = []

    def run(label, command):
        result = subprocess.run(sandbox + command, capture_output=True, timeout=160)
        (work / (label + ".stdout")).write_bytes(result.stdout)
        (work / (label + ".stderr")).write_bytes(result.stderr)
        assert result.returncode == 0, (label, result.returncode, str(work))
        records.append(label)
        print("PASS:", label, flush=True)
        return result.stdout

    # Keep Wine alive until initialization completes. Terminating the namespace
    # after the first short CLI call can interrupt COM proxy registration and
    # make an unchanged GUI fail RegisterDragDrop with E_NOINTERFACE.
    run("wine-initialization", ["/bin/sh", "-c", "wineboot --init && wineserver -w"])

    def cli(label, *args):
        return run(label, ["/usr/bin/wine", "/app/gamepkg.exe", *args])

    assert metadata["version"].encode() in cli("cli-version", "--version")
    assert metadata["version"].encode() in run(
        "gui-version", ["/usr/bin/wine", "/app/" + gui_name, "--version"]
    )
    cli_audio = cli("cli-audio", "check-bundled-audio")
    gui_audio = run(
        "gui-audio", ["/usr/bin/wine", "/app/" + gui_name, "--check-bundled-audio"]
    )
    assert json.loads(cli_audio) == json.loads(gui_audio)
    cli("keygen", "keygen", "-o", "Z:\\state\\creator.key")
    try:
        cli(
            "pack",
            "pack",
            "Z:\\fixture",
            "-o",
            "Z:\\state\\fixture.gamepkg",
            "--key",
            "Z:\\state\\creator.key",
        )
        cli("verify", "verify", "Z:\\state\\fixture.gamepkg")
        cli("install", "install", "Z:\\state\\fixture.gamepkg", "--trust-creator")
        assert b"com.gamepkg.collect-stars" in cli("list", "list")
        if options.gui:
            check_gui(sandbox, gui_name, work, options.drop)
            records.append("gui-default-vulkan-resize-close")
            if options.drop:
                assert b"com.gamepkg.wine-drop-catalog" in cli(
                    "list-after-drop", "list"
                )
                assert not (
                    work / "library/com.gamepkg.wine-drop-catalog.gamepkg"
                ).exists()
                records.append("native-ole-catalog-drop-persisted-without-download")
        (work / "result.json").write_text(
            json.dumps(
                {
                    "archive_sha256": digest,
                    "checks": records,
                    "dummy_audio": True,
                    "actual_windows_hardware": False,
                },
                indent=2,
            )
            + "\n"
        )
        print("PASS: silent packaged Windows Wine checks; evidence:", work, flush=True)
    finally:
        (work / "creator.key").unlink(missing_ok=True)


def check_gui(sandbox, name, work, native_drop):
    # The OLE source owns the GUI process in the SAME Wine instance. Separate
    # bwrap PID/tmp namespaces cannot exchange Wine's marshalled window handles.
    command = ["/usr/bin/wine", "/app/" + name]
    if native_drop:
        command = [
            "/usr/bin/wine",
            "/app/drop-source.exe",
            "Z:\\state\\dropped.gamepkglist",
            "Z:\\app\\" + name,
            "Z:\\state\\drop-go",
        ]
    with (work / "gui-window.log").open("wb") as output:
        child = subprocess.Popen(
            sandbox + command,
            stdout=output,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        try:
            end = time.monotonic() + 60
            window = None
            while time.monotonic() < end:
                assert child.poll() is None, ("GUI exited early", str(work))
                found = subprocess.run(
                    [
                        "xdotool",
                        "search",
                        "--onlyvisible",
                        "--class",
                        "^" + re.escape(name) + "$",
                    ],
                    capture_output=True,
                    text=True,
                    timeout=5,
                )
                windows = found.stdout.split() if found.returncode == 0 else []
                if len(windows) == 1:
                    window = windows[0]
                    break
                time.sleep(0.2)
            assert window, ("Unique GUI window not found", str(work))
            for label, resize in [("library", False), ("library-small", True)]:
                if resize:
                    subprocess.run(
                        ["xdotool", "windowsize", window, "800", "600"],
                        check=True,
                        timeout=5,
                    )
                time.sleep(2)
                identity = subprocess.check_output(
                    ["xprop", "-id", window, "WM_CLASS"], text=True, timeout=5
                )
                assert name in identity, identity
                raw = work / (label + ".xwd")
                subprocess.run(
                    ["xwd", "-silent", "-id", window, "-out", str(raw)],
                    check=True,
                    timeout=5,
                )
                subprocess.run(
                    [
                        "ffmpeg",
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-y",
                        "-i",
                        str(raw),
                        "-frames:v",
                        "1",
                        str(work / (label + ".png")),
                    ],
                    check=True,
                    timeout=10,
                )
            if native_drop:
                perform_drop(window, name, child, work)
            subprocess.run(
                ["xdotool", "windowactivate", "--sync", window], check=True, timeout=5
            )
            assert (
                subprocess.check_output(
                    ["xdotool", "getactivewindow"], text=True, timeout=5
                ).strip()
                == window
            )
            subprocess.run(
                ["xdotool", "key", "--clearmodifiers", "alt+F4"], check=True, timeout=5
            )
            assert child.wait(timeout=20) == 0
            print("PASS: default Vulkan GUI, resize and normal close", flush=True)
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGTERM)
                try:
                    child.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(child.pid, signal.SIGKILL)
                    child.wait()


def perform_drop(window, name, child, work):
    subprocess.run(
        ["xdotool", "windowactivate", "--sync", window], check=True, timeout=5
    )
    assert (
        subprocess.check_output(
            ["xdotool", "getactivewindow"], text=True, timeout=5
        ).strip()
        == window
    )
    identity = subprocess.check_output(
        ["xprop", "-id", window, "WM_CLASS"], text=True, timeout=5
    )
    assert name in identity, identity
    subprocess.run(
        ["xdotool", "mousemove", "--window", window, "700", "550", "mousedown", "1"],
        check=True,
        timeout=5,
    )
    log = work / "gui-window.log"

    def wait_for(needle):
        end = time.monotonic() + 15
        while needle not in log.read_text(errors="replace"):
            assert child.poll() is None, ("Native OLE fixture exited early", str(work))
            assert time.monotonic() < end, (
                "Timed out waiting for " + needle,
                str(work),
            )
            time.sleep(0.1)

    try:
        (work / "drop-go").write_text("go")
        wait_for("Native OLE drag source ready")
        subprocess.run(
            ["xdotool", "mousemove", "--window", window, "400", "200"],
            check=True,
            timeout=5,
        )
        time.sleep(1)
    finally:
        subprocess.run(["xdotool", "mouseup", "1"], check=True, timeout=5)
    wait_for("DoDragDrop hr=")
    assert "DoDragDrop hr=00040100 effect=1" in log.read_text(errors="replace"), str(
        work
    )
    # Allow the original importer worker to commit before normal window close.
    # Persistence is independently checked by the native CLI after that close.
    time.sleep(3)
    print("PASS: actual native OLE catalog file drop", flush=True)


if __name__ == "__main__":
    main()
