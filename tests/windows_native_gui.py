"""Build an admitted Windows native package; optionally verify its real Vulkan window."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

PROJECT = Path(__file__).resolve().parents[1]
GAME_ID = "com.gamepkg.windows-native-image"
INDEXED_ID = "com.gamepkg.windows-native-indexed"
FIXTURES = PROJECT / "tests/fixtures"
sys.path.insert(0, str(PROJECT / "tools"))
from package_release import executable_architecture


def write_recipe(root, game_id, network=False, indexed=False):
    fixture = PROJECT / "examples/minimal"
    (root / "source/src").mkdir(parents=True)
    for name in ("thumbnail.png", "LICENSE", "NOTICE", "CREDITS.md", "source/Cargo.toml", "source/Cargo.lock"):
        shutil.copyfile(fixture / name, root / name)
    mode = "indexed" if indexed else "image"
    shutil.copyfile(FIXTURES / f"windows_native_{mode}.rs", root / "source/src/lib.rs")
    manifest = (fixture / "manifest.toml").read_text()
    manifest = manifest.replace("com.gamepkg.minimal", game_id)
    manifest = manifest.replace("16777216", "536870912").replace("8388608", "268435456")
    if not indexed:
        manifest = manifest.replace("save = false", "save = true")
    if network:
        manifest = manifest.replace('network = "none"', "network = true")
    profile = ""
    if indexed:
        profile = 'native_graphics = "vertex-color-v1"\n'
        (root / "shaders").mkdir()
        shutil.copyfile(FIXTURES / "windows_native_indexed.slang", root / "shaders/scene.slang")
        manifest += """
[[shaders]]
source = "shaders/scene.slang"
entry = "vertexMain"
stage = "vertex"

[[shaders]]
source = "shaders/scene.slang"
entry = "fragmentMain"
stage = "fragment"
"""
    (root / "manifest.toml").write_text(
        f'native_services = true\ntitle = "GamePKG native {mode} fixture"\n' + profile + manifest,
        encoding="utf-8",
    )


def matches_frame(image, expected):
    width, height = image.size
    # The guest draws the middle half of the client. Ignore fractional pixel
    # boundaries, but require solid image pixels and a separate black clear.
    center = image.crop((width * 3 // 8, height * 3 // 8, width * 5 // 8, height * 5 // 8))
    margin = image.crop((width // 16, height // 16, width // 8, height // 8))
    return all(pixel == expected for pixel in center.getdata()) and all(
        pixel == (0, 0, 0) for pixel in margin.getdata()
    )


def wait_frame(window, label, expected, previous_size=None):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        image = window.capture(label)
        if image.size != previous_size and matches_frame(image, expected):
            return
        time.sleep(0.1)
    raise AssertionError(f"missing {label} frame in game child {window.owner_pid}: {window.diagnostics()}")


def verify_window(gui, env, home, proof):
    from windows_owned_window import OwnedGameWindow

    last_played = 0
    for label, initial in (("first", (0, 0, 255)), ("restart", (255, 0, 0))):
        with OwnedGameWindow(gui, ["--play", GAME_ID, "--target", "native"], env, proof, label, child=True) as window:
            assert window.owner_pid != window.app.pid, "launcher window is not the game window"
            wait_frame(window, "initial", initial)
            window.key(0x4D, True, extended=True)
            wait_frame(window, "held-right", (0, 255, 0))
            window.key(0x4D, False, extended=True)
            wait_frame(window, "released", initial)
            before = window.size()
            window.resize(*window.alternate_client_size())
            wait_frame(window, "resized", initial, previous_size=before)
            window.close()
        records = list((home / "progress").glob("*.json"))
        assert len(records) == 1, records
        played = json.loads(records[0].read_text())["played_ms"]
        assert played > last_played, (played, last_played)
        last_played = played
        print(f"Verified visible Windows native Vulkan image window: {label}", flush=True)


def verify_indexed_window(gui, env, proof):
    from windows_owned_window import OwnedGameWindow

    with OwnedGameWindow(gui, ["--play", INDEXED_ID, "--target", "native"], env, proof, "indexed", child=True) as window:
        # Blue vertex data becomes red through the signed fragment shader.
        wait_frame(window, "slang-indexed", (255, 0, 0))
        before = window.size()
        window.resize(*window.alternate_client_size())
        wait_frame(window, "slang-resized", (255, 0, 0), previous_size=before)
        window.close()
    print("Verified visible Windows native Vulkan indexed Slang window", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--visible", action="store_true", help="requires native Windows, hardware Vulkan and an interactive desktop")
    parser.add_argument("--shader-backend", choices=["wgsl", "spirv"], default="wgsl")
    args = parser.parse_args()
    assert os.name == "nt", "native Windows is required"
    cli = PROJECT / "target/release/gamepkg.exe"
    gui = cli.with_name("gamepkg-gui.exe")
    arch = executable_architecture(cli, True)
    assert executable_architecture(gui, True) == arch == os.environ.get("GAMEPKG_TEST_EXPECTED_ARCH", arch)
    with tempfile.TemporaryDirectory(prefix="gamepkg-native-image-") as directory:
        root = Path(directory)
        home = root / "library"
        config = root / "settings.toml"
        settings = "automatic_update_checks=false\ncheck_app_updates_on_startup=false\nmute_audio=true\nmemory_ceiling_mib=512\n"
        config.write_text(settings + f'shader_backend="{args.shader_backend}"\n')
        env = dict(os.environ, GAMEPKG_HOME=str(home), GAMEPKG_CONFIG=str(config))

        def run(*arguments, good=True):
            result = subprocess.run([str(cli), *map(str, arguments)], env=env, capture_output=True, text=True, timeout=2100)
            output = result.stdout + result.stderr
            assert (result.returncode == 0) == good, output
            return output

        key = root / "creator.key"
        run("keygen", "-o", key)
        for game_id, network, indexed in (
            (INDEXED_ID + "-network", True, True),
            (GAME_ID, False, False),
            (INDEXED_ID, False, True),
        ):
            recipe = root / game_id
            write_recipe(recipe, game_id, network, indexed)
            package = root / f"{game_id}.gamepkg"
            run("pack", recipe, "-o", package, "--key", key)
            run("install", package, "--trust-creator")
            build = home / "builds" / hashlib.sha256(package.read_bytes()).hexdigest()
            output = run("build", game_id, "--target", "native", good=not network)
            if network:
                assert "requires offline native services" in output, output
                assert not build.exists(), build
                continue
            binaries = list(build.glob("native-v1-*/compatibility-game.exe"))
            assert len(binaries) == 1 and not list(build.rglob("*.wasm")), build
            binary = binaries[0]
            assert executable_architecture(binary, True) == arch
            receipt = json.loads(binary.with_suffix(".receipt").read_bytes()[64:])
            assert receipt["bootstrap"] == "InProcess"
            expected_count = int(indexed)
            assert len(receipt["shader_preparation"]) == len(receipt["game"]["shaders"]) == expected_count
            assert receipt["binary"] == hashlib.sha256(binary.read_bytes()).hexdigest()
            assert "compiled game verified" in run("integrity-check", game_id)
            if indexed:
                # Changing even to another supported backend invalidates the
                # old receipt. Restore the settings before the window test.
                alternate = "spirv" if args.shader_backend == "wgsl" else "wgsl"
                config.write_text(settings + f'shader_backend="{alternate}"\n')
                output = run("integrity-check", game_id, good=False)
                assert "no valid playable entry point" in output, output
                config.write_text(settings + f'shader_backend="{args.shader_backend}"\n')
                assert "compiled game verified" in run("integrity-check", game_id)
                print(f"Verified signed offline Windows native indexed Slang build ({args.shader_backend}) and stale shader-cache denial: {arch}", flush=True)
            else:
                print(f"Verified signed offline Windows native image build and online-package denial: {arch}", flush=True)

        if args.visible:
            verify_window(gui, env, home, PROJECT / "target/windows-gui-proof")
            verify_indexed_window(gui, env, PROJECT / "target/windows-gui-proof")
            run("integrity-check", GAME_ID)


if __name__ == "__main__":
    main()
