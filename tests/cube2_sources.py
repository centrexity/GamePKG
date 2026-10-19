"""Pinned Cube 2 source integrity and adaptation anchors (maintainer test)."""
import importlib.util
from pathlib import Path
import shutil
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('cube2_build', PROJECT / 'tools/build_cube2_engine.py')
build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build)
lock = build.verify()
assert lock['revision'] == 6889
assert not any(Path(name).suffix in {'.lib', '.a', '.dll', '.exe', '.o'} for name in lock['files'])
with tempfile.TemporaryDirectory(prefix='cube2-integrity-') as temporary:
    source = Path(temporary) / 'source'
    shutil.copytree(build.SOURCE, source)
    def rejected():
        try:
            build.verify(source)
        except ValueError:
            return
        raise AssertionError('Modified source passed integrity verification')
    path = source / 'engine/main.cpp'
    original = path.read_bytes()
    path.write_bytes(original + b'\n// tampered\n')
    rejected()
    path.write_bytes(original)
    extra = source / 'engine/unexpected.cpp'
    extra.write_text('int injected;')
    rejected()
    extra.unlink()
    path.unlink()
    rejected()
    path.symlink_to(build.SOURCE / 'engine/main.cpp')
    rejected()
assert build.replace_once('before', 'before', 'after') == 'after'
for source in ['missing', 'before before']:
    try:
        build.replace_once(source, 'before', 'after')
    except ValueError:
        continue
    raise AssertionError('Changed adaptation anchor accepted')
print('Cube 2 pinned sources: hashes, extra/missing files, symlinks and changed adaptation anchors rejected.')
