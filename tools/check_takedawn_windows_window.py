"""Maintainer-only Windows Vulkan window/keyboard check under Wine.

Uses a fresh private prefix and copied reference saves. No player dependency,
ROM extraction or normal user-library access is involved. Requires a local
X11 display, xdotool, xwd and ffmpeg. Actual Windows-host testing remains
separate from this Wine transport check.
"""

from pathlib import Path
from check_takedawn_paths import require_supported_paths
import argparse
import hashlib
import re
import subprocess
import tempfile
import shutil
import os
import time
import signal
import json

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("executable", type=Path)
parser.add_argument("iso", type=Path)
parser.add_argument("reference_saves", type=Path)
options = parser.parse_args()
executable = options.executable.resolve(strict=True)
iso = options.iso.resolve(strict=True)
reference = options.reference_saves.resolve(strict=True)
if not executable.is_file() or not iso.is_file() or not reference.is_dir():
    parser.error("Executable, ISO file and private reference saves are required")
for tool in ["bwrap", "timeout", "wine", "xdotool", "xwd", "ffmpeg", "xprop"]:
    if not shutil.which(tool):
        parser.error("Missing maintainer tool: " + tool)
if (
    not os.environ.get("DISPLAY")
    or not Path(os.environ.get("XAUTHORITY", "")).is_file()
):
    parser.error("DISPLAY and a readable XAUTHORITY are required")
work = Path(tempfile.mkdtemp(prefix="gamepkg-windows-window."))
for d in ["home", "bin", "saves"]:
    (work / d).mkdir()
name = "gamepkg-window-" + work.name.rsplit(".", 1)[-1] + ".exe"
shutil.copyfile(executable, work / "bin" / name)
shutil.copyfile(executable.parent / "SDL2.dll", work / "bin/SDL2.dll")
for d in ["UDATA", "TDATA"]:
    if (reference / d).exists():
        shutil.copytree(reference / d, work / "saves" / d)
print("Private window evidence:", work, flush=True)
args = [
    "timeout",
    "--signal=TERM",
    "--kill-after=5s",
    "130s",
    "bwrap",
    "--die-with-parent",
    "--unshare-net",
    "--unshare-pid",
    "--clearenv",
]
for p in ["/usr", "/lib", "/lib64", "/etc", "/sys"]:
    args += ["--ro-bind", p, p]
args += [
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
    "--ro-bind",
    "/tmp/.X11-unix",
    "/tmp/.X11-unix",
    "--ro-bind",
    os.environ["XAUTHORITY"],
    "/xauth",
    "--bind",
    str(work),
    "/state",
    "--ro-bind",
    str(work / "bin"),
    "/engine",
    "--ro-bind",
    str(iso),
    "/disc.iso",
    "--chdir",
    "/state",
]
for k, v in {
    "PATH": "/usr/bin:/bin",
    "HOME": "/state/home",
    "USER": "user",
    "LOGNAME": "user",
    "LANG": "C.UTF-8",
    "DISPLAY": os.environ["DISPLAY"],
    "XAUTHORITY": "/xauth",
    "WINEARCH": "win64",
    "WINEPREFIX": "/state/prefix",
    "WINEDEBUG": "-all",
    "WINEDLLOVERRIDES": "mscoree,mshtml=",
    "GAMEPKG_TAKEDAWN_DISC": "Z:\\disc.iso",
    "RECOMP_SAVE_DIR": "Z:\\state\\saves",
    "RECOMP_PB_BACKEND": "vulkan",
    "SDL_AUDIODRIVER": "dummy",
    "RECOMP_FLIP_MARK": "1",
    "RECOMP_STUB_LOG": "1",
    "RECOMP_KEY_TRACE": "1",
    "RECOMP_STDIO_LOG": "Z:\\state\\game.log",
    "BO3_CTL_LOG": "Z:\\state\\ctl.txt",
    "BO3_DRIVE_LOG": "Z:\\state\\drive.txt",
}.items():
    args += ["--setenv", k, v]
args += [
    "/usr/bin/wine",
    "/engine/" + name,
    "--seconds",
    "110",
    "/gamepkg-disc/default.xbe",
]
out = (work / "launcher.log").open("wb")
p = subprocess.Popen(args, stdout=out, stderr=subprocess.STDOUT, start_new_session=True)


def text():
    try:
        return (work / "game.log").read_text(errors="replace")
    except FileNotFoundError:
        return ""


def wait_for(needle, seconds=35):
    end = time.monotonic() + seconds
    while needle not in text():
        if p.poll() is not None:
            raise RuntimeError(f"Process exited {p.returncode} waiting for {needle}")
        if time.monotonic() > end:
            raise RuntimeError("Timed out waiting for " + needle)
        time.sleep(0.1)
    print("Reached:", needle, flush=True)


window = None
held = []
try:
    wait_for("[PRESENT] window up: Vulkan swapchain")
    end = time.monotonic() + 10
    while time.monotonic() < end:
        result = subprocess.run(
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
        ids = result.stdout.split()
        if len(ids) == 1:
            window = ids[0]
            break
        time.sleep(0.2)
    if not window:
        raise RuntimeError("Unique test window not found")
    subprocess.run(
        ["xprop", "-id", window, "WM_CLASS", "_NET_WM_PID"],
        stdout=(work / "window-properties.txt").open("w"),
        check=True,
        timeout=5,
    )
    subprocess.run(
        ["xdotool", "windowactivate", "--sync", window], check=True, timeout=5
    )

    def capture(label):
        if p.poll() is not None:
            raise RuntimeError("Test process exited before window capture")
        identity = subprocess.check_output(
            ["xprop", "-id", window, "WM_CLASS"], text=True, timeout=5
        )
        if name not in identity:
            raise RuntimeError("Test window identity changed")
        raw = work / (label + ".xwd")
        subprocess.run(
            ["xwd", "-silent", "-id", window, "-out", str(raw)], check=True, timeout=5
        )
        subprocess.run(
            [
                "ffmpeg",
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                str(raw),
                "-frames:v",
                "1",
                str(work / (label + ".png")),
            ],
            check=True,
            timeout=10,
        )

    def key(name, hold=0.35):
        if p.poll() is not None:
            raise RuntimeError("Test process exited before keyboard input")
        subprocess.run(
            ["xdotool", "windowactivate", "--sync", window], check=True, timeout=5
        )
        subprocess.run(
            ["xdotool", "windowfocus", "--sync", window], check=True, timeout=5
        )
        active = subprocess.check_output(
            ["xdotool", "getactivewindow"], text=True, timeout=5
        ).strip()
        focus = subprocess.check_output(
            ["xdotool", "getwindowfocus"], text=True, timeout=5
        ).strip()
        print(
            "Keyboard:",
            name,
            "active=" + active,
            "focus=" + focus,
            "target=" + window,
            flush=True,
        )
        if active != window:
            raise RuntimeError("Test window does not own keyboard focus")
        # Establish client-area focus with a click inside the owned window.
        # Keep pointer movement nonblocking; no movement event is needed here.
        subprocess.run(
            ["xdotool", "mousemove", "--window", window, "100", "100", "click", "1"],
            check=True,
            timeout=5,
        )
        time.sleep(0.1)
        # Use real XTest input after checking focus. Always release keys,
        # including after Alt+F4 has destroyed the destination window.
        held.append(name)
        try:
            subprocess.run(["xdotool", "keydown", name], check=True, timeout=5)
            time.sleep(hold)
        finally:
            subprocess.run(["xdotool", "keyup", name], check=True, timeout=5)
            held.remove(name)
        time.sleep(0.35)

    wait_for("Titles30.xmv")
    for i in range(10):
        if "_FEMain.xmv" in text():
            break
        key("Return")
        time.sleep(0.7)
    wait_for("_FEMain.xmv", 15)
    time.sleep(3)
    capture("attract")
    for i in range(5):
        if "SaveMeta.xbx" in text():
            break
        key("Return", 0.6)
        time.sleep(1)
    wait_for("SaveMeta.xbx", 10)
    time.sleep(2)
    for i in range(5):
        if "Profile 1 (disp" in text():
            break
        key("space", 0.4)
        time.sleep(1)
    wait_for("Profile 1 (disp", 10)
    key("space", 0.4)
    time.sleep(3)
    key("space", 0.4)
    time.sleep(3)
    capture("menu")
    subprocess.run(
        ["xdotool", "windowsize", window, "1024", "768"], check=True, timeout=5
    )
    time.sleep(2)
    capture("resized")
    subprocess.run(
        ["xdotool", "getwindowgeometry", "--shell", window],
        stdout=(work / "resized-geometry.txt").open("w"),
        check=True,
        timeout=5,
    )
    key("alt+F4", 0.15)
    code = p.wait(timeout=20)
    assert code == 0, code
    require_supported_paths([work / "game.log"])
    assert "[QUIT] the title's executor is held at a frame" in text()
    geometry = (work / "resized-geometry.txt").read_text()
    assert "WIDTH=1024\n" in geometry and "HEIGHT=768\n" in geometry
    (work / "result.json").write_text(
        json.dumps(
            {
                "exit_code": code,
                "wine_native_window": True,
                "vulkan_swapchain": True,
                "keyboard_profile_navigation": True,
                "resize_requested": [1024, 768],
                "window_id": window,
                "actual_windows_host": False,
                "native_sha256": hashlib.sha256(executable.read_bytes()).hexdigest(),
            },
            indent=2,
        )
        + "\n"
    )
    print(
        "PASS: visible Vulkan window, keyboard profile navigation, resize and normal close",
        flush=True,
    )
finally:
    try:
        for name in held:
            subprocess.run(["xdotool", "keyup", name], check=False, timeout=5)
    finally:
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGTERM)
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL)
                p.wait()
        out.close()
