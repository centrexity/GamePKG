"""Maintainer-only headless GPU display for Linux/Wine window tests.

Runs the given command on a separate Weston/Xwayland desktop. It never connects
its compositor to the user's display. Game/media/audio checks remain the
responsibility of the command; this wrapper does not qualify their results.
"""

import argparse
import json
import os
from pathlib import Path
import secrets
import select
import shutil
import signal
import subprocess
import tempfile
import time


def stop(process):
    if process is None or process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def interrupted(_signal, _frame):
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--evidence", type=Path, help="New directory for display logs")
    parser.add_argument("--vulkan-probe", action="store_true", help="Also record vulkaninfo before the command")
    parser.add_argument("--width", type=int, default=1280)
    parser.add_argument("--height", type=int, default=1024)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not (256 <= args.width <= 8192 and 256 <= args.height <= 8192):
        parser.error("Display dimensions must be between 256 and 8192 pixels")
    command = args.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        parser.error("A test command after -- is required")
    for name in ["weston", "Xwayland", "xdotool", "xprop", "xauth", "dbus-run-session", "metacity"] + (["vulkaninfo"] if args.vulkan_probe else []):
        if not shutil.which(name):
            parser.error("Missing maintainer tool: " + name)
    if args.evidence:
        work = args.evidence.resolve()
        work.mkdir(parents=True, exist_ok=False)
    else:
        work = Path(tempfile.mkdtemp(prefix="gamepkg-gpu-display."))
    print("Private GPU display evidence:", work, flush=True)
    for name in ["home", "data", "cache"]:
        (work / name).mkdir(mode=0o700)
    authority = work / "xauthority"
    authority.touch(mode=0o600)
    # Unix socket addresses have a much shorter limit than normal file paths.
    # Keep this separate from an arbitrarily deep evidence directory.
    runtime = tempfile.TemporaryDirectory(prefix="gamepkg-gpu-runtime.")
    compositor_env = {
        k: v for k, v in os.environ.items() if k in ["PATH", "LANG", "USER", "LOGNAME"]
    }
    compositor_env.update(
        HOME=str(work / "home"),
        XDG_DATA_HOME=str(work / "data"),
        XDG_CACHE_HOME=str(work / "cache"),
        XDG_RUNTIME_DIR=runtime.name,
        PULSE_SERVER="unix:/no-audio-device",
    )
    compositor = server = manager = client = None
    result = {"command_exit": None, "game_qualification": False, "physical_windows_qualification": False}
    previous = signal.signal(signal.SIGTERM, interrupted)
    try:
        with (work / "compositor-launcher.log").open("wb") as log:
            compositor = subprocess.Popen(
                [
                    "weston", "--backend=headless", "--renderer=gl", "--width=" + str(args.width),
                    "--height=" + str(args.height), "--socket=gamepkg-gpu", "--no-config", "--idle-time=0",
                    "--log=" + str(work / "weston.log"),
                ],
                env=compositor_env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
            )
        deadline = time.monotonic() + 20
        while True:
            if compositor.poll() is not None:
                raise RuntimeError("Private GPU compositor exited; inspect " + str(work / "weston.log"))
            if (Path(runtime.name) / "gamepkg-gpu").is_socket():
                break
            if time.monotonic() >= deadline:
                raise RuntimeError("Private Wayland listener did not become ready")
            time.sleep(0.1)
        cookie = secrets.token_hex(16)

        def authorize(display):
            subprocess.run(
                ["xauth", "-f", str(authority)],
                input=f"add {display} MIT-MAGIC-COOKIE-1 {cookie}\n", text=True,
                check=True, capture_output=True, timeout=5,
            )

        # The server loads the cookie before it chooses its display number.
        # Add the matching client lookup after reading that number. Neither
        # operation reads or changes the user's Xauthority file.
        authorize(":0")
        reader, writer = os.pipe()
        try:
            server_env = dict(compositor_env, WAYLAND_DISPLAY="gamepkg-gpu")
            with (work / "xwayland.log").open("wb") as log:
                server = subprocess.Popen(
                    ["Xwayland", "-displayfd", str(writer), "-geometry", f"{args.width}x{args.height}",
                     "-nolisten", "tcp", "-auth", str(authority), "-noreset"],
                    env=server_env, stdout=log, stderr=subprocess.STDOUT,
                    pass_fds=(writer,), start_new_session=True,
                )
        finally:
            os.close(writer)
        try:
            if not select.select([reader], [], [], 20)[0]:
                raise RuntimeError("Private Xwayland server did not become ready")
            display = os.read(reader, 32).decode().strip()
            if not display.isdigit() or server.poll() is not None:
                raise RuntimeError("Private Xwayland server did not report a valid display")
        finally:
            os.close(reader)
        authorize(":" + display)
        # Keep the caller's systemd/audio environment: existing test commands
        # own their memory scopes and explicitly choose dummy/private audio.
        # These window tests use X11 or launch an explicitly configured child.
        client_env = dict(os.environ, DISPLAY=":" + display, XAUTHORITY=str(authority))
        client_env.pop("WAYLAND_DISPLAY", None)
        client_env.pop("WAYLAND_SOCKET", None)
        client_env.pop("SDL_VIDEODRIVER", None)
        deadline = time.monotonic() + 10
        while True:
            probe = subprocess.run(
                ["xdotool", "getdisplaygeometry"], env=client_env, capture_output=True, timeout=2,
            )
            if probe.returncode == 0:
                break
            if compositor.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError("Private Xwayland client connection failed")
            time.sleep(0.1)
        # Rootful Xwayland and a private X11 WM provide keyboard focus even
        # though the headless compositor has no physical keyboard/seat. Its
        # rootless XWM alone cannot deliver XTest keyboard events in that case.
        manager_env = dict(client_env, **compositor_env)
        manager_env.pop("DBUS_SESSION_BUS_ADDRESS", None)
        with (work / "window-manager.log").open("wb") as log:
            manager = subprocess.Popen(
                ["dbus-run-session", "--", "metacity", "--sm-disable", "--no-composite"],
                env=manager_env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True,
            )
        deadline = time.monotonic() + 10
        while True:
            probe = subprocess.run(
                ["xprop", "-root", "_NET_SUPPORTING_WM_CHECK"],
                env=client_env, capture_output=True, text=True, timeout=2,
            )
            if probe.returncode == 0 and "window id # 0x" in probe.stdout:
                break
            if manager.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError("Private X11 window manager did not become ready")
            time.sleep(0.1)
        result["private_display"] = client_env["DISPLAY"]
        result["display_pixels"] = [args.width, args.height]
        result["private_headless_compositor"] = True
        if args.vulkan_probe:
            with (work / "vulkan-info.txt").open("wb") as out, (work / "vulkan-stderr.txt").open("wb") as err:
                probe = subprocess.run(["vulkaninfo"], env=client_env, stdout=out, stderr=err, timeout=30)
            result["vulkaninfo_exit"] = probe.returncode
            if probe.returncode:
                raise RuntimeError("Private display Vulkan probe failed")
        client = subprocess.Popen(command, env=client_env, start_new_session=True)
        result["command_exit"] = client.wait()
        return result["command_exit"]
    finally:
        stop(client)
        stop(manager)
        stop(server)
        stop(compositor)
        runtime.cleanup()
        signal.signal(signal.SIGTERM, previous)
        (work / "result.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    raise SystemExit(main())
