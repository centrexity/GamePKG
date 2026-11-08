"""Asset-free acceptance tests for the native HUD trace/cache auditor."""
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('hud_audit', Path(__file__).resolve().parents[1]
                                               / 'tools/check_takedawn_hud_scaling.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class HudAuditTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.cache = Path(self.temporary.name)
        self.path = self.cache / '948b46bd016af9d0_128x128_x2_sx2.tsc'
        # Synthetic flat color, not game artwork. Checksum independently
        # produced by the pinned native nv2a_texscale_key C implementation.
        self.payload = struct.pack('<I', 0xff112233) * (256 * 256)
        self.header = audit.HEADER.pack(0x53545258, 2, 128, 128, 2, 0,
            0x948b46bd016af9d0, 0x70230397d78cbc17, len(self.payload), 0)
        self.path.write_bytes(self.header + self.payload)
        self.use = '[UISCALE] tex 4f44eea46b9eec9e fmt 0x0f 128x128 levels 1 use 1 scale 0 flip 100'
        self.swap = '[UISCALE] swap 948b46bd016af9d0 128x128 x2: scaled texture in, 0.300 ms on the render thread, flip 101'

    def check(self, lines, factor=2):
        return audit.audit(lines, self.cache, 'crashbreaker', factor)

    def test_cold_and_warm_uploads_require_actual_2d_use(self):
        self.assertEqual(self.check([self.use, self.swap])['scaled_image_flip'], 101)
        self.assertEqual(self.check([self.use.replace('scale 0', 'scale 2')])['scaled_image_flip'], 100)

    def test_missing_wrong_or_unscaled_uploads_are_rejected(self):
        for lines in [[], [self.use], [self.swap],
                      [self.swap, self.use],
                      [self.use.replace('use 1', 'use 2'), self.swap],
                      [self.use.replace('levels 1', 'levels 8'), self.swap],
                      [self.use, self.swap.replace('x2:', 'x3:')],
                      [self.use, self.swap.replace('scaled texture in', 'no result')],
                      [self.use, self.swap.replace('flip 101', 'flip 99')],
                      [self.use.replace('4f44eea46b9eec9e', '0000000000000001'), self.swap],
                      [self.use, self.swap.replace('948b46bd016af9d0', '0000000000000001')]]:
            with self.subTest(lines=lines), self.assertRaises(ValueError):
                self.check(lines)

    def test_warm_replaced_xbox_art_cannot_claim_original_pixels(self):
        # The compressed guest texture keeps the same identity even when a
        # replacement sheet is used. A scale number alone cannot identify it.
        line = '[UISCALE] tex 9181d035e5fbb70e fmt 0x0f 128x64 levels 1 use 1 scale 2 flip 100'
        with self.assertRaisesRegex(ValueError, 'decoded-key upload'):
            audit.audit([line], self.cache, 'xbox-buttons', 2)

    def test_cache_corruption_and_truncation_are_rejected(self):
        for content in [self.header + self.payload[:-1], self.header + self.payload + b'X',
                        bytes([self.header[0] ^ 1]) + self.header[1:] + self.payload,
                        self.header + bytes([self.payload[0] ^ 1]) + self.payload[1:]]:
            self.path.write_bytes(content)
            with self.subTest(size=len(content)), self.assertRaises(ValueError):
                self.check([self.use, self.swap])

    def test_cached_texture_has_to_match_all_identity_fields(self):
        fields = list(audit.HEADER.unpack(self.header))
        for index in [1, 2, 3, 4, 5, 6, 8, 9]:
            changed = fields.copy(); changed[index] ^= 1
            self.path.write_bytes(audit.HEADER.pack(*changed) + self.payload)
            with self.subTest(index=index), self.assertRaises(ValueError):
                self.check([self.use, self.swap])


if __name__ == '__main__':
    unittest.main()
