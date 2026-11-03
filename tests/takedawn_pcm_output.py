"""No playback: reject missing, inserted, reordered or altered device samples."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest
import wave

spec = importlib.util.spec_from_file_location(
    "pcm_output", Path(__file__).resolve().parents[1] / "tools/check_takedawn_pcm_output.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class DevicePcmTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.reference = self.root / "reference.wav"
        self.monitor = self.root / "monitor.pcm"
        # Cross the comparator's read boundary; distinct channels expose swaps.
        self.pcm = b"".join(struct.pack("<hh", i % 30001, -(i % 30001))
                            for i in range(40001))
        self.write_reference(self.pcm)

    def write_reference(self, pcm, channels=2, width=2, rate=48000):
        with wave.open(str(self.reference), "wb") as output:
            output.setparams((channels, width, rate, 0, "NONE", "not compressed"))
            output.writeframes(pcm)

    def test_exact_with_preroll_and_later_audio(self):
        self.monitor.write_bytes(b"\0" * 28 + self.pcm + b"\0" * 64)
        result = module.require_exact_pcm(self.reference, self.monitor, 7)
        self.assertEqual(result["frames"], 40001)

    def test_rejects_sample_edits_and_time_discontinuities(self):
        split = 16384 * 4
        for label, pcm in {
            "inserted silence": self.pcm[:split] + b"\0" * 4 + self.pcm[split:],
            "dropped frame": self.pcm[:split] + self.pcm[split + 4:] + b"\0" * 4,
            "changed channel": self.pcm[:split] + b"\0\0" + self.pcm[split + 2:],
            "swapped blocks": self.pcm[split:] + self.pcm[:split],
            "truncated": self.pcm[:-4],
            "partial frame": self.pcm + b"x",
            "all silence": b"\0" * len(self.pcm),
        }.items():
            with self.subTest(label=label):
                self.monitor.write_bytes(pcm)
                with self.assertRaises(ValueError):
                    module.require_exact_pcm(self.reference, self.monitor, 0)

    def test_rejects_wrong_offset(self):
        self.monitor.write_bytes(self.pcm)
        for offset in (-1, 1, 1000000):
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                module.require_exact_pcm(self.reference, self.monitor, offset)

    def test_rejects_wrong_or_empty_reference(self):
        self.monitor.write_bytes(self.pcm)
        for channels, width, rate in ((1, 2, 48000), (2, 1, 48000), (2, 2, 44100)):
            self.write_reference(self.pcm, channels, width, rate)
            with self.subTest(channels=channels, width=width, rate=rate), self.assertRaises(ValueError):
                module.require_exact_pcm(self.reference, self.monitor, 0)
        self.write_reference(b"")
        with self.assertRaises(ValueError):
            module.require_exact_pcm(self.reference, self.monitor, 0)

    def test_rejects_truncated_wav_payload(self):
        self.monitor.write_bytes(self.pcm)
        self.reference.write_bytes(self.reference.read_bytes()[:-4])
        with self.assertRaisesRegex(ValueError, "reference WAV data is truncated"):
            module.require_exact_pcm(self.reference, self.monitor, 0)


if __name__ == "__main__":
    unittest.main()
