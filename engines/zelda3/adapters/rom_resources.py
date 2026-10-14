"""Decode a selected US ROM into engine resources in RAM, with no output files.

Development bridge while the in-process GamePKG loader is integrated. Only pinned
upstream code may call this; package-supplied Python is never executed.
"""
import builtins
import contextlib
import hashlib
import io
import os
from pathlib import Path
import sys
import threading

USA_SHA1 = '6d4f10a8b10e10dbe624cb23cf03b88bb8252973'
LOCK = threading.Lock()

def decode(rom_bytes):
    if len(rom_bytes) == 1049088:
        rom_bytes = rom_bytes[512:]
    if len(rom_bytes) != 1048576 or hashlib.sha1(rom_bytes).hexdigest() != USA_SHA1:
        raise ValueError('Zelda3 requires the supported USA 1 MiB ROM (optionally with a 512-byte copier header).')
    upstream = Path(__file__).resolve().parents[1] / 'upstream' / 'assets'
    files = {}
    with LOCK:
        old_open, old_mkdir = builtins.open, os.makedirs
        old_path = list(sys.path)
        old_bytecode = sys.dont_write_bytecode
        sys.dont_write_bytecode = True
        sys.path.insert(0, str(upstream))
        class VirtualText(io.StringIO):
            def __init__(self, name):
                super().__init__(); self.name = name
            def write(self, value):
                count = super().write(value); files[self.name] = self.getvalue(); return count
        class VirtualBytes(io.BytesIO):
            def __init__(self, name):
                super().__init__(); self.name = name
            def write(self, value):
                count = super().write(value); files[self.name] = self.getvalue(); return count
        def virtual_open(name, mode='r', *args, **kwargs):
            name = str(name).replace('\\', '/')
            if 'w' in mode:
                return VirtualBytes(name) if 'b' in mode else VirtualText(name)
            if name not in files:
                raise FileNotFoundError('ROM decoder requested an unavailable in-memory resource: ' + name)
            value = files[name]
            return io.BytesIO(value) if 'b' in mode else io.StringIO(value)
        try:
            # Import dependencies before swapping IO; only resource IO is virtualized.
            import util
            rom = util.LoadedRom.__new__(util.LoadedRom)
            rom.ROM = rom_bytes; rom.language = 'us'
            util.ROM = rom
            import extract_resources, extract_music, sprite_sheets, compile_resources
            import yaml
            from PIL import Image
            rom = util.LoadedRom.__new__(util.LoadedRom)
            rom.ROM = rom_bytes; rom.language = 'us'
            util.ROM = rom
            compile_resources.ROM = rom
            builtins.open = virtual_open
            os.makedirs = lambda *args, **kwargs: None
            with contextlib.redirect_stdout(io.StringIO()):
                extract_resources.print_all_text_stuff()
                extract_music.extract_sound_data(rom)
                sprite_sheets.decode_link_sprites()
                sprite_sheets.decode_font()
                class Arguments:
                    sprites_from_png = False
                    languages = None
                    print_assets_header = False
                compile_resources.assets.clear()
                compile_resources.main(Arguments())
            result = bytes(files['../zelda3_assets.dat'])
            if len(result) > 8388608 or len(compile_resources.assets) != 165:
                raise ValueError('Unexpected Zelda3 resource layout or size')
            return result
        finally:
            builtins.open = old_open
            os.makedirs = old_mkdir
            sys.path[:] = old_path
            sys.dont_write_bytecode = old_bytecode
            # Do not retain ROM data or derived resources in imported module caches.
            for name in ['util','extract_resources','extract_music','sprite_sheets','compile_resources',
                         'compile_music','text_compression','tables','sprite_sheet_info','decode_music']:
                sys.modules.pop(name, None)

if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('rom', type=Path, nargs='?')
    parser.add_argument('--stdin', action='store_true')
    args = parser.parse_args()
    if args.stdin:
        data = sys.stdin.buffer.read(1049089)
    else:
        with args.rom.open('rb') as source:
            data = source.read(1049089)
    result = decode(data)
    if args.stdin:sys.stdout.buffer.write(result)
    else:print('Supported ROM decoded in memory:', len(result), 'bytes; 165 resources; no files written.')
