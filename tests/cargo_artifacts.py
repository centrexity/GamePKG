"""Dependency search paths for standalone rustc fixtures, from one Cargo build."""
import json
from pathlib import Path


def dependency_search_args(messages):
    directories = set()
    for line in messages.splitlines():
        message = json.loads(line)
        if message.get('reason') != 'compiler-artifact':
            continue
        for filename in message.get('filenames', []):
            path = Path(filename)
            if path.suffix in {'.rlib', '.rmeta', '.so', '.dylib', '.dll'}:
                directories.add(str(path.parent))
    return [argument for directory in sorted(directories)
            for argument in ('-L', 'dependency=' + directory)]
