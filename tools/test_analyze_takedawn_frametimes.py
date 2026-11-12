import io
import unittest
from analyze_takedawn_frametimes import analyze


def trace(rows):
    return io.StringIO("flip,frame_ms,iv,late\n" + "\n".join(rows))


class FrameAnalysisTests(unittest.TestCase):
    def test_uses_total_elapsed_time_instead_of_averaging_fps(self):
        report = analyze(trace(["1,10,1,0", "2,30,1,1"]), 1, 2, "test")
        self.assertEqual(report["all_selected_frames"]["throughput_hz"], 50)
        self.assertEqual(report["all_selected_frames"]["p99_ms"], 30)
        self.assertEqual(report["all_selected_frames"]["runtime_late_flags"], 1)

    def test_separates_original_half_rate_and_excludes_menus(self):
        report = analyze(trace(["0,1,1,0", "1,16.667,1,0", "2,33.333,2,0",
                                "3,1,1,0"]), 1, 2, "test")
        groups = report["by_original_requested_interval"]
        self.assertEqual(groups["1"]["frames"], 1)
        self.assertEqual(groups["2"]["frames"], 1)
        self.assertEqual(report["all_selected_frames"]["frames"], 2)

    def test_rejects_missing_duplicate_and_unordered_frames(self):
        for rows in (["1,16,1,0", "3,16,1,0"],
                     ["1,16,1,0", "1,16,1,0"],
                     ["2,16,1,0", "1,16,1,0"]):
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                analyze(trace(rows), 1, 3, "test")

    def test_rejects_partial_nonfinite_and_invalid_samples(self):
        for row in ("1,NaN,1,0", "1,inf,1,0", "1,0,1,0", "1,16,1,2",
                    "1,16,9,0", "1,16", "1,-5,1,0"):
            with self.subTest(row=row), self.assertRaises(ValueError):
                analyze(trace([row]), 1, 1, "test")

    def test_requires_actual_boundaries_and_context(self):
        for first, last, context in ((0, 1, "test"), (1, 2, "test"),
                                     (2, 1, "test"), (1, 1, " ")):
            with self.subTest(first=first, last=last), self.assertRaises(ValueError):
                analyze(trace(["1,16,1,0"]), first, last, context)


if __name__ == "__main__":
    unittest.main()
