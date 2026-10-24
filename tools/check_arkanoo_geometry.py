"""Native CRT geometry regression, against unchanged original point captures.

Compiles the actual port's backgrounds and effect functions without the app's
renderer/dependencies. Only unrelated SDK randomness is stubbed; every tested
coordinate and color comes from the actual source and the recorded original.
The full game tests remain separate gates. Python is maintainer orchestration.
"""
import argparse
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
import zlib

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--target')
parser.add_argument('--linker')
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
work = Path(tempfile.mkdtemp(prefix='gamepkg-native-geometry.'))
source = root / 'ports/arkanoo/source/src'
physics = (source / 'physics.rs').read_text()
assert physics.count('pub struct Game {') == 1
# This prefix contains the complete original effect functions and data types,
# before unrelated menu/editor/game-loop integration. Do not copy/reimplement
# their arithmetic in this checker.
(work / 'physics.rs').write_text(physics.split('pub struct Game {')[0])

def path_literal(path):
    return json.dumps(path.as_posix())

def f32(value):
    bits = struct.unpack('<I', struct.pack('<f', value))[0]
    return f'f32::from_bits(0x{bits:08x})'

checks = []
fixture = json.loads((root / 'tests/fixtures/arkanoo-background.json').read_text())
for index, case in enumerate(fixture['cases']):
    expected = work / f'background-{index}.bin'
    expected.write_bytes(zlib.decompress(bytes.fromhex(case['trace_zlib_hex'])))
    checks.append(f'''{{
        let scene = background::campaign({case['level']}, {case['frame']}u64).unwrap();
        assert_eq!(scene.color, {case['background']}u32);
        check(&scene.points, include_bytes!({path_literal(expected)}), "background {index}");
    }}''')
fixture = json.loads((root / 'tests/fixtures/arkanoo-trails.json').read_text())
for index, case in enumerate(fixture['cases']):
    expected = work / f'trail-{index}.bin'
    expected.write_bytes(b''.join(struct.pack('<iiI', *point) for point in case['points']))
    ball = case['ball']
    trail = ','.join('[' + ','.join(f32(value) for value in point) + ']' for point in case['trail'])
    checks.append(f'''{{
        let mut ball = physics::Ball::new({f32(ball[0])} * 0.5 + 3.0,
            {f32(ball[1])} * 0.5 + 63.0);
        ball.vx = {f32(ball[2])} * 0.5;
        ball.vy = {f32(ball[3])} * 0.5;
        ball.trail = vec![{trail}];
        let points = physics::ball_effect_points(&ball, {case['frame']}u64);
        check(&points, include_bytes!({path_literal(expected)}), "trail {index}");
    }}''')
main = '''#![allow(dead_code, unused_imports)]
mod gamepkg { pub fn random_u64() -> u64 { 0 } }
mod levels {}
mod physics;
#[path = BACKGROUND]
mod background;
fn check(points: &[physics::EffectPoint], expected: &[u8], label: &str) {
    let mut actual = Vec::new();
    for point in points {
        actual.extend(point.x.to_le_bytes());
        actual.extend(point.y.to_le_bytes());
        actual.extend(point.color.to_le_bytes());
    }
    assert_eq!(actual.len(), expected.len(), "{label}: point count");
    if let Some(index) = actual.iter().zip(expected).position(|(a,b)| a != b) {
        panic!("{label}: point {} byte {} differs: {} != {}", index / 12, index % 12, actual[index], expected[index]);
    }
}
fn main() {
CHECKS
    println!("PASS: every recorded background/trail coordinate and color matches the original.");
}
'''.replace('BACKGROUND', path_literal(source / 'background.rs')).replace('CHECKS', '\n'.join(checks))
(work / 'main.rs').write_text(main)
binary = work / ('geometry.exe' if 'windows' in (args.target or '') or os.name == 'nt' else 'geometry')
command = ['rustc', '--edition=2021', '-O', str(work / 'main.rs'), '-o', str(binary)]
if args.target:
    command += ['--target', args.target]
if args.linker:
    command += ['-C', 'linker=' + args.linker]
print('Private geometry evidence:', work, flush=True)
subprocess.run(command, check=True)
subprocess.run([str(binary)], check=True)
