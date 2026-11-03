"""Run the complete current ZIP helper boundary test as a real 32-bit process.

Maintainer-only; requires a C++ frontend, a multilib C linker, and 32-bit libc.
No C++ standard library is needed by the production helper or test body.
"""
from pathlib import Path
import argparse
import os
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cc', default='cc')
parser.add_argument('--runtime-libraries', type=Path)
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
source = (project / 'tests/cube2_zip_bounds.cpp').read_text()
# Preserve the entire existing test body. Replace only standard test-header
# definitions, allowing multilib C installations without C++ library headers.
assert source.count('#include <cassert>') == 1
assert source.count('#include <climits>') == 1
source = source.replace('#include <cassert>', '#define assert(expression) do { if (!(expression)) __builtin_trap(); } while (0)')
source = source.replace('#include <climits>', '#define UINT_MAX (~0u)\n#define ULLONG_MAX (~0ull)\n#define LLONG_MAX __LONG_LONG_MAX__\n#define INT_MAX __INT_MAX__')
source = source.replace('"../engines/cube2/adapters/zip_bounds.h"', '"' + str(project / 'engines/cube2/adapters/zip_bounds.h') + '"')
source = '#if __SIZEOF_POINTER__ != 4\n#error Requires actual 32-bit pointers\n#endif\n' + source
with tempfile.TemporaryDirectory(prefix='gamepkg-zip32-') as temporary:
    root = Path(temporary)
    harness = root / 'zip32.cpp'
    harness.write_text(source)
    binary = root / 'zip32'
    subprocess.run([args.cc, '-m32', '-x', 'c++', '-std=c++11', '-O2', '-Wall', '-Wextra', '-Werror', str(harness), '-o', str(binary)], check=True, timeout=60)
    assert binary.read_bytes()[:5] == b'\x7fELF\x01', 'Compiler did not produce ELF32'
    environment = dict(os.environ)
    if args.runtime_libraries:
        environment['LD_LIBRARY_PATH'] = str(args.runtime_libraries.resolve())
    subprocess.run([str(binary)], check=True, env=environment, timeout=30)
print('Actual ELF32 ZIP helper passed all existing extent, entry, LE-read and 1,024-offset scan assertions.')
