"""Maintainer-only independent 200-digit reference; requires mpmath.

The native checker reads the committed fixture with Python's standard library.
Neither the game nor the checker requires mpmath at runtime.
"""
from pathlib import Path
import json
import struct
import mpmath as mp

mp.mp.dps = 200
root = Path(__file__).resolve().parents[1]
inputs = {0, 0x80000000}
for exponent in range(255):
    for mantissa in [0, 0x00555555, 0x007fffff]:
        for sign in [0, 0x80000000]:
            inputs.add(sign | exponent << 23 | mantissa)
for value in [0.3, 0.5, 1, 2, 3.14159265, 1e10, 1e20, 1e30,
              (2.0 ** 64) * struct.unpack('<f', struct.pack('<f', 0.3))[0]]:
    for sign in [1, -1]:
        inputs.add(struct.unpack('<I', struct.pack('<f', sign * value))[0])

def bits(value):
    return struct.unpack('<I', struct.pack('<f', float(value)))[0]

cases = []
for value in sorted(inputs):
    number = struct.unpack('<f', struct.pack('<I', value))[0]
    exact = mp.mpf(number)
    sine = bits(mp.sin(exact)) if value != 0x80000000 else value
    cases.append([value, sine, bits(mp.cos(exact))])
fixture = {'reference': 'mpmath sin/cos at 200 decimal digits, rounded to IEEE binary32',
           'generator_version': mp.__version__, 'cases': cases}
destination = root / 'tests/fixtures/arkanoo-trig-f32.json'
destination.write_text(json.dumps(fixture, indent=2) + '\n')
print(f'Generated {len(cases)} independent trig reference vectors.')
