"""Maintainer-only: require the actual Vulkan executor, not just its request."""
from pathlib import Path
import argparse

ACTIVE = b"RECOMP_PB_BACKEND=vulkan: executor draws through Vulkan"
FALLBACK = b"RECOMP_PB_BACKEND=vulkan: no device, staying on the CPU rasteriser"


def require_vulkan(paths):
    active = False
    fallback = False
    for path in paths:
        overlap = b""
        with Path(path).open("rb") as source:
            while chunk := source.read(65536):
                text = overlap + chunk
                active |= ACTIVE in text
                fallback |= FALLBACK in text
                overlap = text[-max(len(ACTIVE), len(FALLBACK)):]
    if fallback:
        raise ValueError("Vulkan initialization fell back to CPU rendering")
    if not active:
        raise ValueError("the actual Vulkan executor was not confirmed")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("logs", nargs="+", type=Path)
    args = parser.parse_args()
    try:
        require_vulkan(args.logs)
    except (OSError, ValueError) as error:
        parser.exit(1, f"FAIL: {error}\n")
    print("PASS: actual Vulkan executor confirmed; no CPU fallback observed.")


if __name__ == "__main__":
    main()
