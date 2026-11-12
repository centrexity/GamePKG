"""Maintainer CLI provenance regression; requires original r6889 data, never edits it."""
from pathlib import Path
import argparse
import json
import shutil
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--data', type=Path, default=PROJECT/'target/cube2-test-data')
args = parser.parse_args()
cli = PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-upstream-include-check-') as temporary:
    root = Path(temporary)
    shutil.copytree(args.data/'data', root/'data', symlinks=True)
    (root/'packages/textures').mkdir(parents=True)
    (root/'packages/textures/notexture.png').write_bytes(b'fixture')

    def run():
        result = subprocess.run([str(cli), 'check-game-data', str(root), '--json'],
                                capture_output=True, text=True, timeout=30)
        return result.returncode, json.loads(result.stdout)

    target_name = 'data/menus/browse_textures2.cfg'
    _, report = run()
    assert any(issue['reference'] == target_name for issue in report['upstream_missing_references']), 'Select the original pinned r6889 menu fixture'
    menus = root/'data/menus.cfg'
    original = menus.read_bytes()
    menus.write_bytes(original+b'\n// changed\n')
    code, report = run()
    assert code == 1 and not report['upstream_missing_references']
    assert any(issue['reference'] == target_name for issue in report['missing_or_unreadable'])
    menus.write_bytes(original)
    target = root/target_name
    target.symlink_to(root/'data/stdlib.cfg')
    code, report = run()
    assert code == 1 and not report['upstream_missing_references']
    assert any(issue['reference'] == target_name for issue in report['missing_or_unreadable'])
print('CLI provenance passed: exact upstream reference, modified parent and linked target remain distinct; original data unchanged.')
