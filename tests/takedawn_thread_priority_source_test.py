"""Asset-free checks that Windows line endings do not weaken source admission."""
import hashlib
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('thread_check', Path(__file__).resolve().parents[1]
                                               / 'tools/check_takedawn_thread_priority.py')
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class SourceTests(unittest.TestCase):
    def setUp(self):
        self.canonical = b'/* synthetic native source */\nvoid example(void) {}\n'
        self.hash_patch = patch.object(check, 'EXPECTED_SOURCE', hashlib.sha256(self.canonical).hexdigest())
        self.hash_patch.start(); self.addCleanup(self.hash_patch.stop)

    def test_canonical_source_needs_no_git_object(self):
        with patch.object(check.subprocess, 'check_output') as git:
            self.assertEqual(check.reference_bytes(Path('source'), self.canonical), self.canonical)
            git.assert_not_called()

    def test_crlf_checkout_uses_exact_pinned_git_object(self):
        with patch.object(check.subprocess, 'check_output', side_effect=[check.EXPECTED_REVISION+'\n', self.canonical]) as git:
            self.assertEqual(check.reference_bytes(Path('source'), self.canonical.replace(b'\n', b'\r\n')), self.canonical)
            self.assertEqual(git.call_count, 2)

    def test_modified_checkout_cannot_hide_behind_canonical_git_object(self):
        with patch.object(check.subprocess, 'check_output', side_effect=[check.EXPECTED_REVISION+'\n', self.canonical]):
            with self.assertRaisesRegex(RuntimeError, 'beyond Windows line endings'):
                check.reference_bytes(Path('source'), self.canonical.replace(b'\n', b'\r\n') + b'/* changed */')

    def test_wrong_revision_or_changed_git_object_is_rejected(self):
        for replies in [['wrong\n'], [check.EXPECTED_REVISION+'\n', self.canonical+b'/* changed */']]:
            with patch.object(check.subprocess, 'check_output', side_effect=replies):
                with self.assertRaises(RuntimeError):
                    check.reference_bytes(Path('source'), b'not the canonical source')


if __name__ == '__main__':
    unittest.main()
