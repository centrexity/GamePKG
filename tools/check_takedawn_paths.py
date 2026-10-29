"""Maintainer-only: reject reached untranslated instructions and game stubs."""
from pathlib import Path
import argparse

MARKERS = {
    b"[STUB] unresolved 0x": "an unresolved upstream game function was reached",
    b"[UNIMPL] untranslated instruction REACHED:": "an untranslated instruction was reached",
}


def require_supported_paths(paths):
    paths = list(paths)
    if not paths:
        raise ValueError("no runtime logs supplied")
    for path in paths:
        overlap = b""
        with Path(path).open("rb") as source:
            while chunk := source.read(65536):
                text = overlap + chunk
                for marker, reason in MARKERS.items():
                    if marker in text:
                        raise ValueError(f"{path}: {reason}")
                overlap = text[-max(map(len, MARKERS)):]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs="+", type=Path)
    args = parser.parse_args()
    try:
        require_supported_paths(args.logs)
    except (OSError, ValueError) as error:
        parser.exit(1, f"FAIL: {error}\n")
    print("PASS: no reached untranslated instructions or unresolved game functions.")


if __name__ == "__main__":
    main()
