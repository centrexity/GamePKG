"""Maintainer-only real native Windows settings/restart gate under isolated Wine.

Uses the original overlay's SDL input driver and renderer-owned screenshots.
Python/Wine are diagnostic tools, not player requirements. No game-state edits,
ROM extraction, normal user-library access or global keyboard input occur.
"""

import argparse
import hashlib
import json
import os
import shutil
import signal
import subprocess
import tempfile
import time
from pathlib import Path
from check_takedawn_paths import require_supported_paths

parser = argparse.ArgumentParser(description=__doc__)
for field in ["runner", "game", "iso", "saves"]:
    parser.add_argument("--" + field, type=Path, required=True)
options = parser.parse_args()
for field in ["runner", "game", "iso", "saves"]:
    setattr(options, field, getattr(options, field).resolve(strict=True))
if not options.saves.is_dir() or not all(
    p.is_file() for p in [options.runner, options.game, options.iso]
):
    parser.error(
        "Native runner/game, owned ISO and private reference saves are required"
    )
for tool in ["systemd-run", "timeout", "bwrap", "wine", "ffmpeg"]:
    if not shutil.which(tool):
        parser.error("Missing maintainer tool: " + tool)
if (
    not os.environ.get("DISPLAY")
    or not Path(os.environ.get("XAUTHORITY", "")).is_file()
):
    parser.error("DISPLAY and readable XAUTHORITY are required")
work = Path(tempfile.mkdtemp(prefix="gamepkg-windows-restart-window."))
for folder in ["bin", "saves", "home"]:
    (work / folder).mkdir()
shutil.copyfile(options.runner, work / "runner.exe")
name = "gamepkg-restart-" + work.name.rsplit(".", 1)[-1] + ".exe"
shutil.copyfile(options.game, work / "bin" / name)
shutil.copyfile(options.game.parent / "SDL2.dll", work / "bin/SDL2.dll")
control = work / "overlay.txt"
control.write_text("")
for folder in ["UDATA", "TDATA"]:
    src = options.saves / folder
    if src.exists():
        shutil.copytree(src, work / "saves" / folder)
log = work / "window-owner.log"
out = log.open("wb")
cmd = [
    "systemd-run",
    "--user",
    "--scope",
    "--quiet",
    "--collect",
    "--property=MemoryMax=4294967296",
    "--property=MemorySwapMax=0",
    "timeout",
    "--signal=TERM",
    "--kill-after=5s",
    "220s",
    "bwrap",
    "--die-with-parent",
    "--unshare-net",
    "--unshare-pid",
    "--clearenv",
]
for path in ["/usr", "/lib", "/lib64", "/etc", "/sys"]:
    cmd += ["--ro-bind", path, path]
cmd += [
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
    str(options.iso),
    "/disc.iso",
    "--chdir",
    "/state",
]
values = {
    "PATH": "/usr/bin:/bin",
    "LANG": "C.UTF-8",
    "HOME": "/state/home",
    "DISPLAY": os.environ["DISPLAY"],
    "XAUTHORITY": "/xauth",
    "WINEPREFIX": "/state/prefix",
    "WINEARCH": "win64",
    "WINEDEBUG": "-all",
    "WINEDLLOVERRIDES": "mscoree,mshtml=",
    "RECOMP_OVERLAY_TEST": "Z:\\state\\overlay.txt",
}
for key, value in values.items():
    cmd += ["--setenv", key, value]
cmd += [
    "/usr/bin/wine",
    "Z:\\state\\runner.exe",
    "Z:\\state\\bin\\" + name,
    "Z:\\disc.iso",
    "Z:\\state\\saves",
    "Z:\\state\\window-game.log",
    "--window",
    "180",
]
p = subprocess.Popen(cmd, stdout=out, stderr=subprocess.STDOUT, start_new_session=True)
print("Private restart window evidence:", work, flush=True)
seq = 0


def text():
    try:
        return (work / "window-game.log").read_text(errors="replace")
    except FileNotFoundError:
        return ""


def wait(test, timeout=60):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if test():
            return
        if p.poll() is not None:
            raise RuntimeError("Owner exited early: " + log.read_text(errors="replace"))
        time.sleep(0.08)
    raise TimeoutError("Condition did not arrive; retained logs at " + str(work))


def command(line):
    with control.open("a") as f:
        f.write(line + "\n")


def state():
    global seq
    seq += 1
    path = work / f"state-{seq:03}.txt"
    command(f"state Z:\\state\\{path.name}")
    wait(
        lambda: path.exists()
        and path.read_text(errors="replace").endswith("Quit game\n"),
        8,
    )
    return path.read_text()


def choose(prefix):
    for _ in range(65):
        s = state()
        if any(line.startswith("> ") and prefix in line for line in s.splitlines()):
            return s
        command("key down")
    raise RuntimeError("Row not reached: " + prefix + "\n" + s)


def capture(label):
    raw = work / (label + ".bmp")
    command(f"shot Z:\\state\\{raw.name}")
    wait(lambda: raw.exists() and raw.stat().st_size > 54, 10)
    # Ensure the original renderer finished its write before decoding the image.
    wait(lambda: ("window shot: Z:\\state\\" + raw.name) in text(), 10)
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
    return raw.name


try:
    wait(lambda: "[PRESENT] window up:" in text())
    time.sleep(2)
    print("First native game window ready", flush=True)
    command("key f1")
    s = state()
    assert "open 1" in s, s
    command("key pgdn")
    s = choose("setting: Render scale")
    before = s
    command("key right")
    s = state()
    assert "Restart now to apply" in s, s
    wanted = s
    first_window = capture("before-restart")
    choose("action: Restart now to apply")
    command("key enter")
    command("key esc")
    s = state()
    assert (
        "Restart now to apply" in s and "restarted after normal" not in log.read_text()
    )
    command("key enter")
    command("key enter")
    # The original teardown can close/restore its logging tee before the last
    # atexit handler prints. The parent's event requires the restart signal,
    # normal process exit and descendant cleanup; it is the authoritative
    # confirmation when that late C-stdio diagnostic is unavailable. Clear
    # consumed inputs immediately so the next process cannot replay them.
    wait(
        lambda: "GamePKG: managed restart requested; normal exit pending." in text()
        or "Native game restarted after normal shutdown: 1" in log.read_text(),
        25,
    )
    control.write_text("")
    wait(
        lambda: "Native game restarted after normal shutdown: 1" in log.read_text(), 25
    )
    print("Rust owner observed normal shutdown and relaunched", flush=True)
    wait(lambda: text().count("[PRESENT] window up:") == 2, 60)
    time.sleep(2)
    command("key f1")
    after = state()
    assert "open 1" in after, after
    command("key pgdn")
    after = choose("setting: Render scale")
    assert "Restart now to apply" not in after, after
    value = lambda data: next(
        line.split(" = ", 1)[-1]
        for line in data.splitlines()
        if "setting: Render scale" in line
    )
    assert value(before) != value(after), (before, after)
    assert value(wanted) == value(after), (wanted, after)
    second_window = capture("after-restart")
    choose("action: Quit game")
    command("key enter")
    command("key enter")
    code = p.wait(timeout=30)
    assert code == 0, (code, log.read_text())
    data = text()
    owner = log.read_text()
    assert data.count("verified disc admitted in place; 716 files, read-only.") == 2
    assert data.count("[QUIT] the title's executor is held at a frame") == 2
    assert owner.count("Native game restarted after normal shutdown: 1") == 1
    assert "Native game exited: 0" in owner and "Intentional end" not in owner
    require_supported_paths([work / "window-game.log"])
    result = {
        "exit_code": 0,
        "transport": "native Windows executable under Wine",
        "original_overlay_input_driver": True,
        "normal_shutdowns": 2,
        "managed_restarts": 1,
        "iso_admissions": 2,
        "invalid_legacy_iso_filename": True,
        "settings_before": value(before),
        "settings_requested": value(wanted),
        "settings_after_restart": value(after),
        "same_tracked_session": True,
        "first_render_capture": first_window,
        "second_render_capture": second_window,
        "runner_sha256": hashlib.sha256(options.runner.read_bytes()).hexdigest(),
        "native_sha256": hashlib.sha256(options.game.read_bytes()).hexdigest(),
        "actual_windows_hardware": False,
    }
    (work / "window-restart-result.json").write_text(
        json.dumps(result, indent=2) + "\n"
    )
    print(json.dumps(result, indent=2), flush=True)
finally:
    if p.poll() is None:
        os.killpg(p.pid, signal.SIGTERM)
        try:
            p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL)
            p.wait()
    out.close()
