"""Release names derive from actual executables, including on a 64-bit build host."""
from pathlib import Path
import sys, tempfile
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from package_release import executable_architecture
with tempfile.TemporaryDirectory(prefix='gamepkg-architecture-') as directory:
    path = Path(directory) / 'program'
    for machine, elf_class, name in [(3, 1, 'i686'), (62, 2, 'x86_64')]:
        header = bytearray(64); header[:4] = b'\x7fELF'; header[4] = elf_class; header[5] = 1
        header[18:20] = machine.to_bytes(2, 'little'); path.write_bytes(header)
        assert executable_architecture(path, False) == name
        header[4] = 3 - elf_class; path.write_bytes(header)
        try: executable_architecture(path, False)
        except ValueError: pass
        else: raise AssertionError('accepted a mismatched ELF class')
    for machine, name in [(0x14c, 'i686'), (0x8664, 'x86_64')]:
        header = bytearray(128); header[:2] = b'MZ'; header[60:64] = (64).to_bytes(4, 'little')
        header[64:68] = b'PE\0\0'; header[68:70] = machine.to_bytes(2, 'little'); path.write_bytes(header)
        assert executable_architecture(path, True) == name
    for data in [b'', b'MZ', b'\x7fELF', bytes(64)]:
        path.write_bytes(data)
        for windows in [False, True]:
            try: executable_architecture(path, windows)
            except ValueError: pass
            else: raise AssertionError('accepted a malformed executable')
print('Release architecture detection passed: ELF/PE 32-bit and 64-bit, class mismatch, and malformed headers.')
