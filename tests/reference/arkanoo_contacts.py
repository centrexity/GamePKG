"""Maintainer oracle: run the pinned original ball-contact block unchanged."""
from pathlib import Path
import hashlib
import json
import subprocess
import sys
import tempfile

source = Path(sys.argv[1]) / 'src/game.rs'
text = source.read_text()
begin = text.index('// Ball-to-ball collisions (only when not in portal mode)')
end = text.index('// Paddle and block collisions', begin)
fragment = text[begin:end]
cases = [
    (False, [[200.,400.,16.,4.],[210.,400.,-18.,-12.]]),
    (False, [[200.,400.,2.,4.],[212.,400.,-2.,6.]]),
    (False, [[200.,400.,2.,4.],[200.,400.,-2.,6.]]),
    (False, [[200.,400.,2.,4.],[204.,400.,-2.,6.],[208.,400.,18.,20.]]),
    (True, [[200.,400.,16.,4.],[210.,400.,-18.,-12.]]),
    (False, [[200.,400.,16.,4.],[208.,408.,-18.,-12.]]),
    (False, [[200.,400.,16.,4.],[209.,409.,-18.,-12.]]),
]
header = '''const BALL_SIZE:i32=12;
struct Ball { x:f32,y:f32,vel_x:f32,vel_y:f32,active:bool }
struct Color { r:u8,g:u8,b:u8 }
struct Particle;
impl Particle {fn new(_:f32,_:f32,_:f32,_:f32,color:Color)->Self {let _=(color.r,color.g,color.b);Self}}
enum SoundEffect {Bounce}
fn play_sound(_:SoundEffect) {}
struct Game { balls:Vec<Ball>,particles:Vec<Particle>,portal_active:bool }
impl Game {fn collide(&mut self) {'''
body = header + fragment + '}}\nfn main(){\n'
for portal, balls in cases:
    values = ','.join('Ball{x:%sf32,y:%sf32,vel_x:%sf32,vel_y:%sf32,active:true}' % tuple(ball) for ball in balls)
    body += 'let mut g=Game{balls:vec![' + values + '],particles:Vec::new(),portal_active:' + str(portal).lower() + '};g.collide();\n'
    body += 'println!("{}",g.particles.len());for b in &g.balls {println!("{:?}",[b.vel_x,b.vel_y]);}\n'
body += '}\n'
with tempfile.TemporaryDirectory(prefix='arkanoo-contact-reference-') as temporary:
    root = Path(temporary)
    rust = root / 'reference.rs'
    rust.write_text(body)
    binary = root / 'reference'
    subprocess.run(['rustc', '--edition=2021', '-O', str(rust), '-o', str(binary)], check=True)
    lines = iter(subprocess.check_output([str(binary)], text=True).splitlines())
    fixtures = []
    for portal, balls in cases:
        particles = int(next(lines))
        expected = [json.loads(next(lines)) for _ in balls]
        fixtures.append(dict(portal=portal, balls=balls, expected=expected, particles=particles))
    print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',
                         fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(), fixtures=fixtures), indent=2))
