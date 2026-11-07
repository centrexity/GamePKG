#!/usr/bin/env python3
"""Maintainer analysis of completed native frametime traces, not display latency.

Select an observed driving range explicitly: menus/loading must not improve the
reported gameplay result. Keep the title's requested presentation intervals
separate so intentional 30 Hz cameras cannot conceal missed 60 Hz deadlines.
Players do not need this script or Python.
"""
import argparse
import csv
import json
import math
from pathlib import Path
import statistics


def percentile(values, fraction):
    """Nearest-rank percentile, including the worst sample for tiny datasets."""
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def summarize(rows):
    values = [row["frame_ms"] for row in rows]
    elapsed = math.fsum(values)
    return {
        "frames": len(values),
        "elapsed_ms": elapsed,
        "throughput_hz": len(values) * 1000 / elapsed,
        "mean_ms": elapsed / len(values),
        "median_ms": statistics.median(values),
        "p95_ms": percentile(values, .95),
        "p99_ms": percentile(values, .99),
        "max_ms": max(values),
        "over_20ms": sum(value > 20 for value in values),
        "over_33_333ms": sum(value > 1000 / 30 for value in values),
        "runtime_late_flags": sum(row["late"] for row in rows),
    }


def analyze(source, first, last, context):
    if first < 0 or last < first or not context.strip():
        raise ValueError("provide a valid driving range and measurement context")
    selected = []
    previous = -1
    for line, row in enumerate(csv.DictReader(source), 2):
        try:
            flip, interval, late = (int(row[key]) for key in ("flip", "iv", "late"))
            duration = float(row["frame_ms"])
            if (flip <= previous or interval not in (0, 1, 2, 3, 4)
                    or late not in (0, 1) or not math.isfinite(duration) or duration <= 0):
                raise ValueError("invalid or unordered timing sample")
        except (KeyError, TypeError, ValueError) as error:
            raise ValueError(f"invalid trace row {line}: {error}") from error
        previous = flip
        if first <= flip <= last:
            selected.append({"flip": flip, "iv": interval, "late": late,
                             "frame_ms": duration})
    if not selected or selected[0]["flip"] != first or selected[-1]["flip"] != last:
        raise ValueError("trace does not cover both driving boundaries")
    if len(selected) != last - first + 1:
        raise ValueError("driving range contains missing frames")
    return {
        "context": context,
        "measurement": "native runtime frame intervals; not on-glass or physical input latency",
        "percentile_method": "nearest rank",
        "first_flip": first,
        "last_flip": last,
        "all_selected_frames": summarize(selected),
        "by_original_requested_interval": {
            str(interval): summarize([row for row in selected if row["iv"] == interval])
            for interval in sorted({row["iv"] for row in selected})
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("trace", type=Path)
    parser.add_argument("--first-flip", type=int, required=True)
    parser.add_argument("--last-flip", type=int, required=True)
    parser.add_argument("--context", required=True,
                        help="platform, GPU, window/headless, audio and competing workload")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        with args.trace.open(newline="") as source:
            report = analyze(source, args.first_flip, args.last_flip, args.context)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(f"Analyzed {report['all_selected_frames']['frames']} driving frames; {args.output}")


if __name__ == "__main__":
    main()
