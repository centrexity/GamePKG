#!/usr/bin/env python3
"""Audit original HUD scaling from a private runtime trace and local cache.

Maintainer only. Reads no ISO and writes no artwork. Identities below were
checked against the owned ISO in place using the pinned native texture decoder.
A positive result covers these icons only, not complete gameplay or frame pace.
"""
import argparse
import json
from pathlib import Path
import re
import struct

# Original compressed texture hash, decoded scaler key, original dimensions.
ICONS = {
    'crashbreaker': ('4f44eea46b9eec9e', '948b46bd016af9d0', 128, 128),
    'crash-boost': ('1997e9cd32ee01c4', '03ecefb7e803c360', 128, 128),
    'crash-x2': ('3efb72afd008d804', '68ac5e67c647f7c6', 128, 128),
    'crash-x4': ('b3dddbc451e9599a', '6ba60a97f075a323', 128, 128),
    'crash-skull': ('012546ada65bf890', '7647727117c76807', 128, 128),
    'crash-stealer': ('b93b194102094e10', '3824318d289902af', 128, 128),
}
MASK = (1 << 64) - 1
HEADER = struct.Struct('<IIIIIIQQII')


def pixel_key(payload, width, height):
    value = 0x9E3779B97F4A7C15 ^ ((width << 32) | height)
    for (pixel,) in struct.iter_unpack('<I', payload):
        value = ((value ^ pixel) * 0xFF51AFD7ED558CCD) & MASK
        value ^= value >> 29
    value ^= value >> 33
    value = (value * 0xC4CEB9FE1A85EC53) & MASK
    value ^= value >> 33
    return value or 1


def audit(lines, cache_dir, icon, factor):
    if factor not in (2, 3, 4):
        raise ValueError('factor must be 2, 3 or 4')
    raw, decoded, width, height = ICONS[icon]
    use_re = re.compile(r'\[UISCALE\] tex ' + raw + r' fmt 0x0f '
                        + f'{width}x{height}'
                        + r' levels (\d+) use (\d+) scale (\d+) flip (\d+)\s*$')
    swap_re = re.compile(r'\[UISCALE\] swap ' + decoded + ' '
                         + f'{width}x{height} x{factor}'
                         + r': scaled texture in, [\d.]+ ms on the render thread, flip (\d+)\s*$')
    first_use = None
    uploaded_at = None
    for line in lines:
        match = use_re.search(line)
        if match:
            levels, use, scale, flip = map(int, match.groups())
            if levels == 1 and use & 1:
                if first_use is None:
                    first_use = flip
                if scale == factor:
                    uploaded_at = flip
        match = swap_re.search(line)
        if match and first_use is not None and int(match[1]) >= first_use:
            uploaded_at = int(match[1])
    if first_use is None or uploaded_at is None:
        raise ValueError(f'{icon}: no observed 2D bind and matching scaled GPU image')
    cache = cache_dir / f'{decoded}_{width}x{height}_x{factor}_sx2.tsc'
    expected_bytes = width * height * factor * factor * 4
    if cache.stat().st_size != HEADER.size + expected_bytes:
        raise ValueError(f'{icon}: cache length differs')
    with cache.open('rb') as file:
        fields = HEADER.unpack(file.read(HEADER.size))
        magic, version, w, h, scale, algo, key, checksum, size, pad = fields
        if ((magic, version, w, h, scale, algo, key, size, pad)
                != (0x53545258, 2, width, height, factor, 0,
                    int(decoded, 16), expected_bytes, 0)):
            raise ValueError(f'{icon}: cache identity differs')
        payload = file.read(expected_bytes)
    if pixel_key(payload, width * factor, height * factor) != checksum:
        raise ValueError(f'{icon}: cache checksum differs')
    return {'icon': icon, 'factor': factor, 'first_2d_bind_flip': first_use,
            'scaled_image_flip': uploaded_at, 'output_width': width * factor,
            'output_height': height * factor, 'cache_bytes': HEADER.size + expected_bytes,
            'cache_identity_and_pixel_checksum': 'pass'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--log', required=True, type=Path)
    parser.add_argument('--cache', required=True, type=Path)
    parser.add_argument('--icon', action='append', choices=ICONS)
    parser.add_argument('--factor', type=int, choices=[2, 3, 4], default=2)
    args = parser.parse_args()
    if args.log.stat().st_size > 64 * 1024 * 1024:
        parser.error('trace exceeds the 64 MiB audit bound')
    lines = args.log.read_text(errors='replace').splitlines()
    results = [audit(lines, args.cache, icon, args.factor)
               for icon in (args.icon or ['crashbreaker'])]
    print(json.dumps({'result': 'pass-observed-local-HUD-scaling',
                      'scope': 'Selected icons, renderer trace and local cache only; '
                               'no complete gameplay or performance qualification.',
                      'artwork_written_by_auditor': False, 'icons': results}, indent=2))


if __name__ == '__main__':
    main()
