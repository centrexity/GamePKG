"""Select exact Android test executables from Cargo's JSON artifact stream.

Maintainer-only qualification tooling; Android players do not need Python.
"""
import json
from pathlib import Path
import re
import sys

SUITES = (
    "components", "portability", "runtime", "render_resources", "memory_budget",
    "audio_effect_assets", "music_permissions", "game_arkanoo",
)


def select(artifacts, manifest):
    expected = {name: [] for name in (*SUITES, "library")}
    for line in artifacts.read_text(encoding="utf-8").splitlines():
        item = json.loads(line)
        if item.get("reason") != "compiler-artifact" or item.get("profile", {}).get("test") is not True:
            continue
        if Path(item.get("manifest_path", "")).resolve() != manifest:
            continue
        target = item.get("target", {})
        kinds = target.get("kind", [])
        if "lib" in kinds:
            name = "library"
        elif "test" in kinds and target.get("name") in SUITES:
            name = target["name"]
        else:
            continue
        executable = item.get("executable")
        if not executable:
            continue
        path = Path(executable).resolve()
        if not path.is_file() or not re.fullmatch(r"[A-Za-z0-9_-]+", path.name):
            raise ValueError(f"Invalid executable for {name}: {path}")
        expected[name].append(path)
    for name, paths in expected.items():
        if len(paths) != 1:
            raise ValueError(f"Expected one Cargo-reported {name} test executable; found {len(paths)}")
    return [paths[0] for paths in expected.values()]


if __name__ == "__main__":
    paths = select(Path(sys.argv[1]), Path(sys.argv[2]).resolve())
    for path in paths:
        sys.stdout.buffer.write(str(path).encode("utf-8") + b"\0")
