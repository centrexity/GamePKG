"""Maintainer-only: compare a captured device stream with the game's PCM tap."""
import argparse
import hashlib
from pathlib import Path
import wave


def require_exact_pcm(reference, monitor, offset_frames):
    if offset_frames < 0:
        raise ValueError("monitor offset must be nonnegative")
    monitor = Path(monitor)
    if monitor.stat().st_size % 4:
        raise ValueError("monitor must contain complete stereo s16le frames")
    digest = hashlib.sha256()
    frames = 0
    with wave.open(str(reference), "rb") as source, monitor.open("rb") as output:
        if (source.getnchannels(), source.getsampwidth(), source.getframerate(),
                source.getcomptype()) != (2, 2, 48000, "NONE"):
            raise ValueError("reference must be uncompressed 48000 Hz stereo s16le")
        expected = source.getnframes()
        if not expected:
            raise ValueError("reference must contain audio frames")
        output.seek(offset_frames * 4)
        while chunk := source.readframes(16384):
            if len(chunk) % 4:
                raise ValueError("reference contains a partial audio frame")
            actual = output.read(len(chunk))
            if len(actual) != len(chunk):
                raise ValueError(f"monitor is truncated at reference frame {frames}")
            if actual != chunk:
                first = next(i for i, (a, b) in enumerate(zip(actual, chunk)) if a != b)
                raise ValueError(f"device PCM differs at reference frame {frames + first // 4}")
            digest.update(chunk)
            frames += len(chunk) // 4
        if frames != expected:
            raise ValueError("reference WAV data is truncated")
    return {"frames": frames, "offset_frames": offset_frames,
            "sample_rate": 48000, "pcm_sha256": digest.hexdigest()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("monitor", type=Path)
    parser.add_argument("--offset-frames", type=int, required=True,
                        help="measured recording preroll, in stereo frames")
    args = parser.parse_args()
    try:
        result = require_exact_pcm(args.reference, args.monitor, args.offset_frames)
    except (OSError, ValueError, EOFError, wave.Error) as error:
        parser.exit(1, f"FAIL: {error}\n")
    print(f"PASS: all {result['frames']} device frames match; PCM SHA-256 "
          f"{result['pcm_sha256']}. This proves only the supplied capture.")


if __name__ == "__main__":
    main()
