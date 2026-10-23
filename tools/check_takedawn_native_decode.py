"""Maintainer oracle only: compare native layout/decoding against the pinned
original Python tools on the full private retail function inventory. Players
do not run or depend on this script. No XBE or game assets are written to disk.
Both decoders share Capstone; this checks the native model/CFG port, not
independent proof that Capstone or the original game analysis is correct.
"""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--native', type=Path, required=True)
parser.add_argument('--disc', type=Path, required=True)
parser.add_argument('--toolkit', type=Path, required=True)
parser.add_argument('--functions', type=Path, required=True)
parser.add_argument('--mode', choices=['linear', 'cfg'], default='linear')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
lock = json.loads((project / 'engines/takedawn/upstream-lock.json').read_text())
commit = subprocess.check_output(['git', '-C', str(args.toolkit), 'rev-parse', 'HEAD'], text=True).strip()
if commit != lock['toolkit']['commit']:
    raise RuntimeError('reference toolkit is not pinned')
if args.functions.stat().st_size > 16 * 1024 * 1024:
    raise RuntimeError('reference metadata exceeds its bound')
functions = json.loads(args.functions.read_text())
if len(functions) != 24000:
    raise RuntimeError('expected the complete pinned 24,000-function disassembly inventory')
sys.path.insert(0, str(args.toolkit))
from tools.xiso.xdvdfs import Xiso
from tools.xbe_parser.xbe_parser import XBEParser
from tools.recomp.disasm import Disassembler
from tools.recomp.config import FORCE_CODE_SECTIONS, FORCE_DATA_SECTIONS

with Xiso(args.disc) as disc:
    executable = disc.read(disc.find('default.xbe'))
if hashlib.sha256(executable).hexdigest() != lock['disc_manifest']['executable_sha256']:
    raise RuntimeError('independent reference XBE hash does not match')
fd = os.memfd_create('gamepkg-native-decode-oracle', os.MFD_ALLOW_SEALING)
try:
    with os.fdopen(os.dup(fd), 'wb') as out:
        out.write(executable)
    fcntl.fcntl(fd, fcntl.F_ADD_SEALS,
                fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE | fcntl.F_SEAL_SEAL)
    image = XBEParser(f'/proc/{os.getpid()}/fd/{fd}').parse()
finally:
    os.close(fd)

def digest(value):
    return hashlib.sha256(json.dumps(value, default=vars, sort_keys=True,
                                    separators=(',', ':'), ensure_ascii=True).encode('ascii')).hexdigest()

start_time = time.monotonic()
with tempfile.TemporaryFile(mode='w+t', encoding='utf8') as native_report:
    subprocess.run([str(args.native), f'reference-{args.mode}', str(args.disc), str(args.functions)],
                   stdout=native_report, check=True, timeout=180)
    native_seconds = time.monotonic() - start_time
    native_report.seek(0)
    native_image = json.loads(next(native_report))
    wanted = {
        'kind': 'image', 'base_address': image.header.base_address,
        'image_size': image.header.image_size, 'entry_point': image.header.entry_point,
        'kernel_thunk': image.header.kernel_thunk_addr,
        'sections': [{
            'name': s.name, 'virtual_address': s.virtual_addr, 'virtual_size': s.virtual_size,
            'raw_address': s.raw_addr, 'raw_size': s.raw_size, 'flags': s.flags,
            'is_code': s.name not in FORCE_DATA_SECTIONS and (bool(s.flags & 4) or s.name in FORCE_CODE_SECTIONS),
        } for s in image.sections],
    }
    if native_image != wanted:
        raise RuntimeError(f'native XBE layout differs: {native_image!r} vs {wanted!r}')
    decoder = Disassembler()
    total_instructions = total_blocks = decoded_functions = 0
    rejected = []
    reference_start = time.monotonic()
    for index, function in enumerate(functions):
        start, end = int(function['start'], 16), int(function['end'], 16)
        section = next((s for s in image.sections if s.virtual_addr <= start
                        and end <= s.virtual_addr + min(s.virtual_size, s.raw_size)
                        and s.name not in FORCE_DATA_SECTIONS
                        and (s.flags & 4 or s.name in FORCE_CODE_SECTIONS)), None)
        if section is None:
            expected = {'kind': 'rejected_extent', 'start': start, 'end': end,
                        'reason': 'not_backed_executable_span'}
            rejected.append({'start': hex(start), 'end': hex(end)})
        else:
            offset = section.raw_addr + start - section.virtual_addr
            raw = executable[offset:offset + end - start]
            instructions = (decoder.disassemble_cfg(raw, start, end, [start]) if args.mode == 'cfg'
                            else decoder.disassemble_function(raw, start, end))
            blocks = decoder.build_basic_blocks(instructions, start, end)
            expected = {
                'kind': 'function', 'start': start, 'end': end,
                'instructions': len(instructions), 'blocks': len(blocks),
                'instruction_sha256': digest(instructions), 'block_sha256': digest(blocks),
            }
            total_instructions += len(instructions)
            total_blocks += len(blocks)
            decoded_functions += 1
        line = native_report.readline()
        if not line:
            raise RuntimeError(f'native report stopped before function {index} ({start:#x})')
        actual = json.loads(line)
        if actual != expected:
            raise RuntimeError(f'native decode/block mismatch at {start:#x}: {actual!r} vs {expected!r}')
        if (index + 1) % 4000 == 0:
            print(f'Compared {index + 1}/24000 functions', flush=True)
    if native_report.read():
        raise RuntimeError('native report contains unaccounted functions')
    print(json.dumps({
        'reference_candidates': len(functions), 'decoded_functions': decoded_functions,
        'mode': args.mode,
        'rejected_extents': rejected, 'sections': len(image.sections),
        'instructions': total_instructions, 'blocks': total_blocks,
        'native_seconds_including_disc_admission': round(native_seconds, 3),
        'reference_comparison_seconds': round(time.monotonic() - reference_start, 3),
        'complete_native_c_generation': False,
    }, sort_keys=True))
