"""Check real PE import layouts, runtime rejection and malformed addresses."""
from pathlib import Path
import struct
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from windows_pe_imports import imports, require_standalone_crt


def image(bits, names, delay=False):
    data = bytearray(4096)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 60, 128)
    data[128:132] = b'PE\0\0'
    optional_size = 240 if bits == 64 else 224
    struct.pack_into('<HH', data, 132, 0x8664 if bits == 64 else 0x14c, 1)
    struct.pack_into('<H', data, 148, optional_size)
    optional = 152
    struct.pack_into('<H', data, optional, 0x20b if bits == 64 else 0x10b)
    struct.pack_into('<I', data, optional + 60, 512)
    directory = optional + (112 if bits == 64 else 96)
    struct.pack_into('<I', data, directory - 4, 16)
    section = optional + optional_size
    struct.pack_into('<IIII', data, section + 8, 3584, 0x1000, 3584, 512)
    size = 32 if delay else 20
    struct.pack_into('<II', data, directory + (13 if delay else 1) * 8,
                     0x1000, (len(names) + 1) * size)
    for index, name in enumerate(names):
        descriptor = 512 + index * size
        if delay:
            struct.pack_into('<II', data, descriptor, 1, 0x1400 + index * 128)
        else:
            struct.pack_into('<I', data, descriptor + 12, 0x1400 + index * 128)
        encoded = name.encode('ascii') + b'\0'
        data[1536 + index * 128:1536 + index * 128 + len(encoded)] = encoded
    return data


with tempfile.TemporaryDirectory(prefix='gamepkg-windows-crt-') as directory:
    path = Path(directory) / 'program.exe'
    for bits in [32, 64]:
        for delay in [False, True]:
            path.write_bytes(image(bits, ['KERNEL32.dll', 'msvcrt.dll'], delay))
            assert require_standalone_crt(path) == ['kernel32.dll', 'msvcrt.dll']
            for runtime in ['VCRUNTIME140.dll', 'VCRUNTIME140_1.dll', 'MSVCP140.dll', 'msvcr120.dll', 'libstdc++-6.dll', 'libwinpthread-1.dll', 'libgcc_s_seh-1.dll']:
                path.write_bytes(image(bits, ['KERNEL32.dll', runtime], delay))
                assert runtime.lower() in imports(path)
                try:
                    require_standalone_crt(path)
                except ValueError as error:
                    assert runtime.lower() in str(error)
                else:
                    raise AssertionError('accepted external compiler runtime')
            broken = image(bits, ['KERNEL32.dll'], delay)
            struct.pack_into('<I', broken, 512 + (4 if delay else 12), 0xffff0000)
            path.write_bytes(broken)
            try:
                imports(path)
            except ValueError:
                pass
            else:
                raise AssertionError('accepted an unbacked DLL name')
            for length in [0, 63, 150, 512, 1600]:
                path.write_bytes(image(bits, ['KERNEL32.dll'], delay)[:length])
                try:
                    imports(path)
                except ValueError:
                    pass
                else:
                    raise AssertionError('accepted truncated PE')
print('PE32/PE32+ normal and delay imports: system CRT allowed, external runtimes and malformed addresses rejected.')
