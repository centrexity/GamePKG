"""Maintainer oracle: compile the unchanged upstream force-law fragment."""
from pathlib import Path
import hashlib,json,subprocess,sys,tempfile
source=Path(sys.argv[1])/'src/entities.rs'
text=source.read_text();begin=text.index('// Apply gravity mode physics');end=text.index('self.x += self.vel_x;',begin)
fragment=text[begin:end]
cases=[(False,4.,2.,6.,10),(True,4.,2.,6.,20),(True,4.,8.9,6.,2),(True,4.,-4.,6.,10),(True,4.,12.,6.,20),(False,-3.,-2.,-7.,25)]
with tempfile.TemporaryDirectory(prefix='arkanoo-force-reference-') as temporary:
 root=Path(temporary);rust=root/'reference.rs'
 body='struct Ball {vel_x:f32,vel_y:f32,spin:f32}\nimpl Ball {fn forces(&mut self,gravity_mode:bool){'+fragment+'}}\nfn main(){\n'
 for gravity,x,y,spin,steps in cases:
  body+=f'let mut b=Ball{{vel_x:{x}f32,vel_y:{y}f32,spin:{spin}f32}};for _ in 0..{steps}{{b.forces({str(gravity).lower()});}}println!("{{:?}}",[b.vel_x,b.vel_y,b.spin]);\n'
 rust.write_text(body+'}\n');binary=root/'reference'
 subprocess.run(['rustc','--edition=2021','-O',str(rust),'-o',str(binary)],check=True)
 output=subprocess.check_output([str(binary)],text=True).splitlines()
 fixtures=[]
 for case,line in zip(cases,output):
  gravity,x,y,spin,steps=case
  fixtures.append({'gravity':gravity,'velocity_spin':[x,y,spin],'steps':steps,'expected':json.loads(line)})
 print(json.dumps({'revision':'9d613e78c9ed5657ae7fda9206cd4a82a893b7c8','entities_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'fragment_sha256':hashlib.sha256(fragment.encode()).hexdigest(),'fixtures':fixtures},indent=2))
