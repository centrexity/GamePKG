#!/usr/bin/env python3
"""Maintainer-only RGB region measurements against an explicit reference frame.

Unlike adjacent-frame masks, a persistent two-frame change is still measured.
Camera motion, overlays and diagnostic effects also change pixels: these counts
identify frame changes, not a texture, root cause, fix or performance result.
Pillow is needed only for this maintainer CLI, never by players.
"""
import argparse
import json
from pathlib import Path


def validate_rectangle(size, rectangle):
    x, y, width, height = rectangle
    if (min(x, y) < 0 or min(width, height) <= 0
            or x + width > size[0] or y + height > size[1]):
        raise ValueError("region must fit completely within the image")
    return x, y, x + width, y + height


def measure_rgb(reference, pixels, threshold=24):
    if (not reference or len(reference) % 3 or len(reference) != len(pixels)
            or not 0 <= threshold <= 255):
        raise ValueError("provide equal, nonempty RGB regions and a byte threshold")
    changed = absolute = white = magenta = 0
    for i in range(0, len(reference), 3):
        delta = [abs(reference[i + k] - pixels[i + k]) for k in range(3)]
        changed += max(delta) > threshold
        absolute += sum(delta)
        r, g, b = pixels[i:i + 3]
        white += min(r, g, b) > 170
        magenta += r > 180 and b > 180 and g < 100
    return {"pixels": len(reference) // 3, "changed_pixels": changed,
            "mean_absolute_rgb_change": absolute / len(reference),
            "white_pixels": white, "magenta_pixels": magenta}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path, help="Explicit, independently selected baseline")
    parser.add_argument("frames", nargs="+", type=Path)
    parser.add_argument("--region", type=int, nargs=4, required=True,
                        metavar=("X", "Y", "WIDTH", "HEIGHT"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    from PIL import Image
    try:
        with Image.open(args.reference) as image:
            size = image.size
            box = validate_rectangle(size, args.region)
            reference = image.convert("RGB").crop(box).tobytes()
        frames = []
        for path in args.frames:
            with Image.open(path) as image:
                if image.size != size:
                    raise ValueError(f"frame dimensions differ: {path}")
                rgb = image.convert("RGB").crop(box).tobytes()
            frames.append({"frame": str(path), **measure_rgb(reference, rgb)})
    except (OSError, ValueError) as error:
        parser.error(str(error))
    report = {"reference": str(args.reference), "image_size": size,
              "region": args.region, "difference_threshold": 24,
              "scope": "Pixel observations only; reference validity and scene alignment require independent review.",
              "frames": frames}
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Measured {len(frames)} frames against the selected reference")


if __name__ == "__main__":
    main()
