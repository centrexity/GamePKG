"""Build admitted native source packages on Linux without requiring a GPU."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from windows_native_gui import write_recipe


PROJECT = Path(__file__).resolve().parents[1]
CLI = Path(os.environ.get("GAMEPKG_TEST_BINARY_DIR", PROJECT / "target/release")) / "gamepkg"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--indexed", action="store_true")
    parser.add_argument("--textured", action="store_true")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="gamepkg-native-ci-") as directory:
        root = Path(directory)
        config, library = root / "settings.toml", root / "library"
        env = dict(os.environ, GAMEPKG_CONFIG=str(config), GAMEPKG_HOME=str(library))

        def run(*arguments, good=True):
            result = subprocess.run(
                [str(CLI), *map(str, arguments)], env=env, capture_output=True, text=True, timeout=900
            )
            output = result.stdout + result.stderr
            assert (result.returncode == 0) == good, output
            return output

        key = root / "test.key"
        run("keygen", "-o", key)
        if args.textured:
            game_id = "com.example.native-textured-ci"
        elif args.indexed:
            game_id = "com.example.native-indexed-ci"
        else:
            game_id = "com.example.native-image-ci"
        source = root / "source"
        write_recipe(source, game_id, indexed=args.indexed, textured=args.textured)
        package = root / "fixture.gamepkg"
        run("pack", source, "-o", package, "--key", key)
        run("install", package, "--trust-creator")
        cache = library / "builds" / hashlib.sha256(package.read_bytes()).hexdigest()
        if args.textured:
            # The host-reviewed native profile is Vulkan/WGSL or Vulkan/SPIR-V
            # only. GLSL/OpenGL stays on the ordinary 2D shader service path.
            config.write_text('automatic_update_checks=false\nshader_backend="glsl"\n')
            output = run("build", game_id, good=False)
            assert "native raster shaders require WGSL/SPIR-V" in output, output
        previous = None
        for backend in ("wgsl", "spirv") if args.indexed or args.textured else ("wgsl",):
            config.write_text(f'automatic_update_checks=false\nshader_backend="{backend}"\n')
            if previous is not None:
                assert "no valid playable entry point" in run("integrity-check", game_id, good=False)
            run("build", game_id, "--target", "native")
            binaries = list(cache.glob("native-v1-*/compatibility-game"))
            assert len(binaries) == 1 and not list(cache.rglob("*.wasm")), cache
            binary = binaries[0]
            contents = binary.read_bytes()
            assert contents.startswith(b"\x7fELF")
            receipt = json.loads(binary.with_suffix(".receipt").read_bytes()[64:])
            assert receipt["bootstrap"] == "InProcess"
            assert receipt["binary"] == hashlib.sha256(contents).hexdigest()
            assert len(receipt["game"]["shaders"]) == len(
                receipt["shader_preparation"]
            ) == int(args.indexed or args.textured)
            assert "compiled game verified" in run("integrity-check", game_id)
            if previous is not None:
                assert receipt["shader_preparation"] != previous
            previous = receipt["shader_preparation"]
            profile = (
                "textured Slang"
                if args.textured
                else "indexed Slang" if args.indexed else "image"
            )
            print(f"Verified signed native {profile} build ({backend})")
        # Integrity failures remain fatal even when the signed package stays
        # intact. Never run a corrupted executable to exercise this branch.
        with binary.open("ab") as output:
            output.write(b"corrupt")
        assert "integrity mismatch" in run("integrity-check", game_id, good=False)


if __name__ == "__main__":
    main()
