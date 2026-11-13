"""Compile a real transitive Rust dependency graph in separated artifact folders."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from cargo_artifacts import dependency_search_args


class CargoSearchPaths(unittest.TestCase):
    def test_transitive_libraries_in_separate_directories(self):
        with tempfile.TemporaryDirectory(prefix='gamepkg cargo artifacts ') as temporary:
            root = Path(temporary)
            leaf_dir = root / 'leaf output'
            middle_dir = root / 'middle output'
            leaf_dir.mkdir()
            middle_dir.mkdir()
            leaf = leaf_dir / 'libfixture_leaf.rlib'
            middle = middle_dir / 'libfixture_middle.rlib'
            (root / 'leaf.rs').write_text('pub fn value() -> u32 { 42 }')
            (root / 'middle.rs').write_text('pub fn value() -> u32 { fixture_leaf::value() }')
            (root / 'probe.rs').write_text('#[test] fn actual_dependency() { assert_eq!(fixture_middle::value(), 42); }')
            def compile_rust(*args):
                return subprocess.run(['rustc', '--edition=2021', *map(str, args)],
                                      capture_output=True, text=True)
            for name, output, extra in [('fixture_leaf', leaf, []),
                                        ('fixture_middle', middle, ['--extern', 'fixture_leaf=' + str(leaf)])]:
                result = compile_rust('--crate-name', name, '--crate-type=rlib',
                                      root / ('leaf.rs' if name == 'fixture_leaf' else 'middle.rs'),
                                      '-o', output, *extra)
                self.assertEqual(result.returncode, 0, result.stderr)
            messages = '\n'.join(json.dumps(message) for message in [
                {'reason': 'compiler-artifact', 'filenames': [str(middle)]},
                {'reason': 'build-script-executed', 'filenames': [str(root / 'stale/libfake.rlib')]},
                {'reason': 'compiler-artifact', 'filenames': [str(leaf), str(root / 'ignored.d')]},
            ])
            command = ['--test', root / 'probe.rs', '--extern', 'fixture_middle=' + str(middle)]
            negative = compile_rust(*command, '-L', 'dependency=' + str(middle_dir), '-o', root / 'negative')
            self.assertNotEqual(negative.returncode, 0)
            self.assertIn('fixture_leaf', negative.stderr)
            search = dependency_search_args(messages)
            self.assertNotIn('dependency=' + str(root / 'stale'), search)
            result = compile_rust(*command, *search, '-o', root / 'probe')
            self.assertEqual(result.returncode, 0, result.stderr)
            run = subprocess.run([str(root / 'probe')], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            self.assertIn('1 passed; 0 failed;', run.stdout)


if __name__ == '__main__':
    unittest.main()
