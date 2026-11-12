"""Maintainer-only, exact-site native SSE control adaptation before C2Rust."""
import hashlib

SITES = [
    ('001DD63F', 'stmxcsr dword ptr [esp + 0xc]',
     'MEM32(esp + 0xC) = gamepkg_takedawn_stmxcsr();'),
    ('001DD652', 'ldmxcsr dword ptr [esp + 8]',
     'gamepkg_takedawn_ldmxcsr(MEM32(esp + 8));'),
]
EXPORTS = ['gamepkg_takedawn_stmxcsr', 'gamepkg_takedawn_ldmxcsr']


def adapt(text):
    seen = []
    for address, instruction, replacement in SITES:
        original = f'RECOMP_UNIMPL("{instruction}", 0x{address}u); /* TODO: {instruction} */'
        count = text.count(original)
        if count > 1:
            raise RuntimeError('duplicate native MXCSR adaptation site: ' + address)
        if count:
            text = text.replace(original, replacement)
            seen.append(address)
    if seen:
        if len(seen) != len(SITES):
            raise RuntimeError('incomplete native MXCSR setup pair')
        text = ('extern unsigned int gamepkg_takedawn_stmxcsr(void);\n'
                'extern void gamepkg_takedawn_ldmxcsr(unsigned int);\n' + text)
    return text, seen


def receipt(project, adapted_units):
    if adapted_units != {'recomp_0004_018': ['001DD63F', '001DD652']}:
        raise RuntimeError('expected exactly one complete native MXCSR setup pair')
    source = project / 'engines/takedawn/rust-port/native_mxcsr.rs'
    return {'format': 'gamepkg-takedawn-native-mxcsr-v1',
            'implementation_sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
            'adapted_units': adapted_units, 'native_exports': EXPORTS}
