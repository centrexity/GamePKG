"""Maintainer check of the actual upstream ZIP stream with staged adaptations.

Only its surrounding archive/container types are replaced by a tiny fixture.
The complete stream implementation and local-header reader come from the pinned
source. No engine media or user library is read.
"""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--upstream', action='store_true', help='reproduce the unadapted output-size failure')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
source = (root / 'engines/cube2/upstream/shared/zip.cpp').read_text()
if not args.upstream:
    for patch in json.loads((root / 'engines/cube2/adapters/zip-bounds-patches.json').read_text()):
        assert source.count(patch['before']) == 1, 'ZIP adaptation anchor changed'
        source = source.replace(patch['before'], patch['after'])
header = source[source.index('static bool readlocalfileheader('):source.index('static vector<ziparchive *> archives;')]
stream = source[source.index('struct zipstream : stream'):source.index('stream *openzipfile(')]
fixture = (root / 'tests/cube2_zip_stream.cpp').read_text()
assert fixture.count('// ORIGINAL_LOCAL_HEADER_AND_STREAM') == 1
fixture = fixture.replace('// ORIGINAL_LOCAL_HEADER_AND_STREAM', header + '\n' + stream)
with tempfile.TemporaryDirectory(prefix='gamepkg-zip-stream-') as temporary:
    directory = Path(temporary)
    cpp = directory / 'stream.cpp'
    cpp.write_text(fixture)
    binary = directory / 'stream'
    subprocess.run(['g++', '-std=c++11', '-O1', '-g', '-Wall', '-Wextra', '-Werror', '-fsanitize=address,undefined', '-I', str(root / 'engines/cube2/adapters'), str(cpp), '-lz', '-o', str(binary)], check=True)
    environment = dict(os.environ, ASAN_OPTIONS='detect_leaks=1', UBSAN_OPTIONS='halt_on_error=1')
    subprocess.run([str(binary)], env=environment, check=True, timeout=30)
print('Original ZIP stream passed bounded output, chunking, seeking and stored/deflated byte checks.')
