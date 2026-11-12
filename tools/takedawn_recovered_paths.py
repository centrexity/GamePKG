"""Maintainer-only recovery using the pinned original native translator.

The supported executable is read from the ISO into sealed RAM. This adds
complete, explicitly bounded functions without changing any existing function
boundary or introducing a runtime instruction decoder.
"""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import sys


def recover(project, source, toolkit, disc):
    lock = json.loads((project / 'engines/takedawn/upstream-lock.json').read_text())
    path = project / 'engines/takedawn/rust-port/recovered-paths.json'
    spec = json.loads(path.read_text())
    if spec['format'] != 'gamepkg-takedawn-recovered-paths-v1':
        raise RuntimeError('unsupported recovery specification')
    sys.path.insert(0, str(toolkit))
    from tools.xiso.xdvdfs import Xiso
    from tools.abi_analysis.xbe_min import XbeFile
    from tools.recomp import config
    from tools.recomp.translator import FunctionTranslator, load_label_db
    from capstone import Cs, CS_ARCH_X86, CS_MODE_32
    with Xiso(disc) as image:
        executable = image.read(image.find('default.xbe'))
    digest = hashlib.sha256(executable).hexdigest()
    if digest != lock['disc_manifest']['executable_sha256']:
        raise RuntimeError('recovery requires the supported executable')
    fd = os.memfd_create('gamepkg-native-path-recovery', os.MFD_ALLOW_SEALING)
    try:
        with os.fdopen(os.dup(fd), 'wb') as stream:
            stream.write(executable)
        fcntl.fcntl(fd, fcntl.F_ADD_SEALS, fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_GROW
                    | fcntl.F_SEAL_WRITE | fcntl.F_SEAL_SEAL)
        image = XbeFile(f'/proc/self/fd/{fd}')
        config.configure_from_xbe(f'/proc/self/fd/{fd}')
        analysis = source / 'build/xr'
        functions = json.loads((analysis / 'disasm/functions.json').read_text())
        database = {int(f['start'], 16): dict(f, end=int(f['end'], 16)) for f in functions}
        decoder = Cs(CS_ARCH_X86, CS_MODE_32)
        records = []
        for entry in spec['functions']:
            start, end, reference = [int(entry[k], 16) for k in ['start', 'end', 'reference']]
            if start >= end or start in database:
                raise RuntimeError('recovery overlaps an existing function entry')
            if any(lo < end and f['end'] > start for lo, f in database.items()):
                raise RuntimeError('recovery would truncate an existing function')
            section = image.find_section(start)
            if section is None or section.name != '.text' or not section.flags & 4:
                raise RuntimeError('recovery target is outside the original code section')
            raw = image.read_bytes_at_va(start, end - start)
            instructions = list(decoder.disasm(raw, start))
            if (not instructions or sum(i.size for i in instructions) != end - start
                    or instructions[-1].mnemonic != 'ret'):
                raise RuntimeError('recovery body is not completely decoded through RET')
            branch = list(decoder.disasm(image.read_bytes_at_va(reference, 5), reference, count=1))
            if (len(branch) != 1 or branch[0].mnemonic != 'jmp'
                    or branch[0].op_str != hex(start)):
                raise RuntimeError('expected direct native branch does not target the recovered body')
            # Decode every local branch to an instruction boundary. Calls may
            # use existing native dispatch; this is not speculative CFG repair.
            boundaries = {i.address for i in instructions}
            for instruction in instructions:
                if instruction.mnemonic.startswith('j'):
                    if int(instruction.op_str, 16) not in boundaries:
                        raise RuntimeError('recovered body has an external or misaligned branch')
            database[start] = {'start': entry['start'], 'end': end,
                               'name': f'sub_{start:08X}', 'size': end - start}
            records.append(dict(entry, sha256=hashlib.sha256(raw).hexdigest(),
                                instructions=len(instructions)))
        translator = FunctionTranslator(executable, database,
                                        load_label_db(str(analysis / 'disasm/labels.json')))
        code = '#define RECOMP_GENERATED_CODE\n#include "recomp_funcs.h"\n#include <math.h>\n'
        for entry in records:
            start = int(entry['start'], 16)
            body = translator.translate_function(start, database[start])
            if (body is None or 'RECOMP_UNIMPL' in body or 'dead code, label not in function' in body):
                raise RuntimeError('original translator did not fully translate the recovered body')
            code += '\n' + body
        return code, {'executable_sha256': digest, 'functions': records,
                      'specification_sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                      'original_media_extracted_to_disk': False}
    finally:
        os.close(fd)


def remove_recovered_stubs(text, records):
    for entry in records:
        name = f"sub_{int(entry['start'], 16):08X}"
        pattern = rf'void {name}\(void\)\s*\{{[^{{}}]*\}}'
        text, count = re.subn(pattern, '', text)
        if count != 1:
            raise RuntimeError('expected exactly one original stub for ' + name)
    return text
