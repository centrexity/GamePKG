"""Compare SDL effect-loading and original streamed-music decoding privately.

Usage: python3 tools/check_sdl_music_reference.py /path/to/original/music
Emits metadata only; all captured PCM is deleted with the temporary directory.
"""
import argparse
import array
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("music", type=Path)
parser.add_argument("--trace", action="store_true", help="verify original SDL stream format and first 16 puts (Linux)")
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
flags = subprocess.check_output(
    ["pkg-config", "--cflags", "--libs", "SDL2_mixer", "sdl2"], text=True
).split()
paths = sorted(args.music.glob("*.mp3"))
assert paths, "no MP3 recordings"


def pcm(path):
    values = array.array("h")
    values.frombytes(path.read_bytes())
    if sys.byteorder != "little":
        values.byteswap()
    return values


with tempfile.TemporaryDirectory(prefix="gamepkg-sdl-music-") as temporary:
    root = Path(temporary)
    for source in ["reference", "music_reference"]:
        subprocess.run(
            ["cc", "-std=c11", str(project / "tools/audio-decoder" / (source + ".c")),
             "-o", str(root / source), *flags], check=True
        )
    trace_env = dict(os.environ)
    if args.trace:
        trace = root / "stream_trace.so"
        subprocess.run(["cc", "-std=c11", "-shared", "-fPIC",
                        str(project / "tools/audio-decoder/stream_trace.c"),
                        "-o", str(trace), *flags, "-ldl"], check=True)
        trace_env["LD_PRELOAD"] = str(trace)
    for path in paths:
        assert path.is_file() and not path.is_symlink()
        info = json.loads(subprocess.check_output([
            "ffprobe", "-v", "quiet", "-show_entries", "stream=sample_rate", "-of", "json", str(path)
        ]))
        source_rate = int(info["streams"][0]["sample_rate"])
        for rate in sorted({source_rate, 44100}):
            traced_format = []
            traced_puts = []
            for executable in ["reference", "music_reference"]:
                command = [str(root / executable), str(path.resolve()), str(rate)]
                if executable == "music_reference":
                    command.append(str(root / "disk.raw"))
                with (root / (executable + ".s16")).open("wb") as output:
                    result = subprocess.run(command, stdout=output, stderr=subprocess.PIPE,
                                            env=trace_env if executable == "music_reference" else None,
                                            check=True, timeout=35)
                if args.trace and executable == "music_reference":
                    for line in result.stderr.decode().splitlines():
                        if line.startswith("TRACE_FORMAT "):
                            traced_format.append([int(v) for v in line.split()[1:]])
                        elif line.startswith("TRACE_PUT "):
                            traced_puts.append(int(line.split()[1]))
                    assert traced_format == [[32784, 2, source_rate, 32784, 2, rate]], traced_format
                    assert traced_puts == [4096] * 16, traced_puts
            chunk = pcm(root / "reference.s16")
            music = pcm(root / "music_reference.s16")
            count = min(len(chunk), len(music))
            different = sum(a != b for a, b in zip(chunk, music))
            maximum = max((abs(a - b) for a, b in zip(chunk, music)), default=0)
            tail_nonzero = sum(v != 0 for v in music[len(chunk):])
            # Require the codec oracle to match the original Music API at both
            # the source rate and original 44.1 kHz mixer rate. Final callback
            # padding is explicit; it must contain only silence.
            assert len(music) >= len(chunk) and different == 0 and tail_nonzero == 0, {
                "name": path.name, "rate": rate, "chunk_samples": len(chunk),
                "music_samples": len(music), "different": different,
                "maximum_delta": maximum, "tail_nonzero": tail_nonzero,
                "first_difference": next((i for i, (a, b) in enumerate(zip(chunk, music)) if a != b), None)
            }
            print(json.dumps({
                "name": path.name, "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                "source_rate": source_rate, "output_rate": rate,
                "chunk_samples": len(chunk), "music_samples": len(music),
                "compared_samples": count, "different": different, "maximum_delta": maximum,
                "music_tail_nonzero": tail_nonzero,
                "traced_format": traced_format, "traced_first_put_bytes": traced_puts,
                "music_compared_sha256": hashlib.sha256((root / "music_reference.s16").read_bytes()[:count * 2]).hexdigest()
            }), flush=True)
