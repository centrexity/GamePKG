"""Maintainer-only decoder build; packaged users need no compiler or Python."""
from pathlib import Path
import argparse
import json
import os
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--github-env', action='store_true')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
result = subprocess.run(['cargo', 'build', '--locked', '--release', '--target', 'wasm32-unknown-unknown',
                         '--manifest-path', 'tools/audio-decoder/Cargo.toml', '--message-format=json'],
                        cwd=project, env=dict(os.environ, GAMEPKG_AUDIO_MAX_MEMORY_BYTES='6291456'),
                        capture_output=True, text=True, timeout=300)
assert result.returncode == 0, result.stderr
artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
wasm = next(Path(path).resolve() for item in artifacts if item.get('reason') == 'compiler-artifact'
            for path in item.get('filenames', []) if path.endswith('.wasm'))
if args.github_env:
    with Path(os.environ['GITHUB_ENV']).open('a', encoding='utf-8') as output:
        output.write(f'GAMEPKG_BUNDLED_AUDIO_DECODER={wasm}\n')
print(wasm)
