import unittest
from takedawn_windows_cpu import patch_kernel_bridge

SOURCE = '''#include "kernel.h"
/* original code */
void xbox_log_thread_role(const char *role, uint32_t routine)
{
    original_thread_registration(role, routine);
}
'''
HEADER = '#define GAMEPKG_NATIVE_WINDOWS_CPU_H\n/* reviewed implementation */\n'


class WindowsCpuPatchTests(unittest.TestCase):
    def test_inserts_host_policy_only_at_the_named_role(self):
        patched = patch_kernel_bridge(SOURCE, HEADER)
        self.assertIn('original_thread_registration(role, routine);', patched)
        self.assertIn('if (role && !strcmp(role, "nv2a-ack"))', patched)
        self.assertEqual(patched.count('gamepkg_windows_pin_executor();'), 1)
        self.assertEqual(patched.count('#if defined(_WIN32)'), 2)

    def test_rejects_already_patched_source(self):
        with self.assertRaises(ValueError):
            patch_kernel_bridge(patch_kernel_bridge(SOURCE, HEADER), HEADER)

    def test_rejects_missing_or_ambiguous_insertion_points(self):
        for source in ['', SOURCE + SOURCE, SOURCE.replace('kernel.h', 'different.h'),
                       SOURCE.replace('uint32_t routine', 'uint64_t routine')]:
            with self.subTest(source=source), self.assertRaises(ValueError):
                patch_kernel_bridge(source, HEADER)

    def test_rejects_missing_implementation(self):
        with self.assertRaises(ValueError):
            patch_kernel_bridge(SOURCE, '')


if __name__ == '__main__':
    unittest.main()
