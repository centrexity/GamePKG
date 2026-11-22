"""Verify that signed but unmigrated DOOM cannot build, play or populate a compiler exchange."""

from pathlib import Path
import os
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get("GAMEPKG_TEST_BINARY_DIR", PROJECT / "target/release")) / "gamepkg"

with tempfile.TemporaryDirectory(prefix="gamepkg-doom-admission-") as directory:
    root = Path(directory)
    config = root / "settings.toml"
    config.write_text("automatic_update_checks=false\n")
    library = root / "library"
    env = dict(os.environ, GAMEPKG_HOME=str(library), GAMEPKG_CONFIG=str(config))

    def run(*args, good=True):
        result = subprocess.run(
            [str(CLI), *map(str, args)], capture_output=True, text=True, env=env, timeout=180
        )
        output = result.stdout + result.stderr
        assert (result.returncode == 0) == good, output
        return output

    key, package = root / "creator.key", root / "doom.gamepkg"
    run("keygen", "-o", key)
    run("pack", PROJECT / "ports/doom", "-o", package, "--key", key)
    run("verify", package)
    run("install", package, "--trust-creator")
    game = "com.gamepkg.doom"
    assert "not built" in run("integrity-check", game)
    store = root / "request.gamepkgstore"
    for command in [
        ("build", game, "--target", "native"),
        ("build", game, "--target", "wasm"),
        ("run", game, "--target", "native"),
        ("run", game, "--target", "wasm", "--frames", "1"),
        ("prepare-build-worker", game, store),
    ]:
        assert "requires migration" in run(*command, good=False), command
    assert not store.exists() and not (library / "builds").exists()
    print("Signed unmigrated DOOM is rejected before build, play or compiler exchange.")
