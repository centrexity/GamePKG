"""Maintainer controls must select a player without accepting pad-file syntax."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from takedawn_input_probe import Bridge, pad_sequence


class ProbeTests(unittest.TestCase):
    def test_physical_ps2_player_two_has_independent_pedal_and_steering(self):
        command = dict(action="step", steer="left", steer_ms=150,
                       drive_ms=800, throttle=255, boost=True)
        self.assertEqual(pad_sequence(command, player=2, scheme="ps2"), [
            "p2-start:150", "p2-a=255+black+lleft:150",
            "p2-a=255+black:650", "p2-start:150"])
        self.assertEqual(pad_sequence(command, player=1), [
            "start:150", "rt=255+a+lleft:150", "rt=255+a:650", "start:150"])

    def test_rejects_untrusted_pad_syntax_and_wrong_player(self):
        for player in [0, 3, True, "2", "2-start\na:999999"]:
            with self.assertRaises(ValueError):
                pad_sequence(dict(action="step"), player=player)
        for change in [dict(steer="left\np2-a:999999"), dict(drive_ms=True),
                       dict(drive_ms=1201), dict(steer="left", steer_ms=401),
                       dict(throttle=-1), dict(throttle=256), dict(boost=1),
                       dict(action="write_memory"), dict(address=0x10000)]:
            with self.assertRaises(ValueError):
                pad_sequence(dict(dict(action="step"), **change))

    def test_wrong_player_pause_and_finished_player_reject_without_writes(self):
        class Owner:
            def poll(self):
                return None
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "run").mkdir()
            target = root / "run/pad.txt"
            target.write_text("retained\n")
            probe = Bridge(root, Owner(), player=2, scheme="ps2")
            for state in [dict(p1_phase=(3, 10), p2_phase=(2, 10), paused_by=(0, 20)),
                          dict(p1_phase=(2, 10), p2_phase=(3, 10), paused_by=(1, 20))]:
                probe.state = lambda state=state: state
                with self.assertRaises(RuntimeError):
                    probe.step(dict(action="step"))
                self.assertEqual(target.read_text(), "retained\n")


if __name__ == "__main__":
    unittest.main()
