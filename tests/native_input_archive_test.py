"""The player Linux build leaves the media archive option implicit."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class MediaArchiveResolution(unittest.TestCase):
    def run_case(self, system, override=False, missing=False, directory=False):
        with tempfile.TemporaryDirectory(prefix='gamepkg-media-path-') as temp:
            root = Path(temp)
            bridge = root / 'host with spaces'
            bridge.mkdir()
            shutil.copyfile(ROOT / 'engines/takedawn/media-bridge/native_input.cmake', bridge / 'native_input.cmake')
            default = bridge / 'target/release/libgamepkg_takedawn_media.a'
            explicit = root / 'chosen archive.a'
            archive = explicit if override else default
            if not missing:
                archive.parent.mkdir(parents=True, exist_ok=True)
                if directory:
                    archive.mkdir()
                else:
                    archive.write_bytes(b'path resolution fixture; not linked')
            script = root / 'check.cmake'
            lines = [f'set(CMAKE_SYSTEM_NAME {system})', f'include([=[{(bridge / "native_input.cmake").as_posix()}]=])']
            if override:
                lines += [f'set(GAMEPKG_TAKEDAWN_MEDIA_ARCHIVE [=[{explicit.as_posix()}]=])']
            lines += ['gamepkg_resolve_media_archive(resolved)',
                      f'if(NOT resolved STREQUAL [=[{archive.as_posix()}]=])',
                      'message(FATAL_ERROR "Wrong archive selected")', 'endif()']
            script.write_text('\n'.join(lines) + '\n')
            return subprocess.run(['cmake', '-P', str(script)], capture_output=True, text=True, timeout=20)

    def test_default_linux_player_path(self):
        self.assertEqual(self.run_case('Linux').returncode, 0)

    def test_explicit_linux_archive_overrides_default(self):
        self.assertEqual(self.run_case('Linux', override=True).returncode, 0)

    def test_windows_uses_explicit_archive(self):
        self.assertEqual(self.run_case('Windows', override=True).returncode, 0)

    def test_windows_does_not_silently_choose_linux_archive(self):
        result = self.run_case('Windows')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Set GAMEPKG_TAKEDAWN_MEDIA_ARCHIVE', result.stderr)

    def test_missing_archive_and_directory_are_rejected(self):
        for system, override, missing, directory in [('Linux', False, True, False),
                                                      ('Windows', True, True, False),
                                                      ('Linux', False, False, True)]:
            with self.subTest(system=system, override=override, directory=directory):
                result = self.run_case(system, override, missing, directory)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('Build the GamePKG media-bridge Rust static library first', result.stderr)


if __name__ == '__main__':
    unittest.main()
