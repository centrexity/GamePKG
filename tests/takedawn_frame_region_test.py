"""Synthetic regression tests: persistent changes must not disappear from audits."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('regions', Path(__file__).resolve().parents[1]
                                            / 'tools/analyze_takedawn_frame_region.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class RegionTests(unittest.TestCase):
    def test_two_frame_disappearance_and_recovery(self):
        # Both consecutive missing frames differ from the independent baseline,
        # even though each has an identical immediate neighbor.
        baseline = bytes([230, 230, 230, 40, 40, 40])
        missing = bytes([40, 40, 40, 40, 40, 40])
        results = [audit.measure_rgb(baseline, f) for f in
                   [baseline, missing, missing, baseline]]
        self.assertEqual([r['changed_pixels'] for r in results], [0, 1, 1, 0])
        self.assertEqual([r['white_pixels'] for r in results], [1, 0, 0, 1])

    def test_threshold_and_tint(self):
        self.assertEqual(audit.measure_rgb(bytes([0, 0, 0]), bytes([24, 0, 0]))
                         ['changed_pixels'], 0)
        result = audit.measure_rgb(bytes([0, 0, 0]), bytes([255, 0, 255]))
        self.assertEqual(result['changed_pixels'], 1)
        self.assertEqual(result['magenta_pixels'], 1)
        self.assertEqual(result['mean_absolute_rgb_change'], 170)

    def test_reject_incomplete_or_mismatched_rgb(self):
        for a, b in [(b'', b''), (b'12', b'12'), (b'123', b'123456')]:
            with self.assertRaises(ValueError):
                audit.measure_rgb(a, b)
        with self.assertRaises(ValueError):
            audit.measure_rgb(b'123', b'123', -1)

    def test_no_padded_outside_image_regions(self):
        self.assertEqual(audit.validate_rectangle((8, 6), (1, 2, 7, 4)), (1, 2, 8, 6))
        for rectangle in [(-1, 0, 1, 1), (0, 0, 0, 1), (7, 0, 2, 1), (0, 5, 1, 2)]:
            with self.assertRaises(ValueError):
                audit.validate_rectangle((8, 6), rectangle)


if __name__ == '__main__':
    unittest.main()
