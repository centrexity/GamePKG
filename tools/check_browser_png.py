"""Maintainer-only PNG pixel parity; no generated fixtures enter the repository."""
from pathlib import Path
import argparse,hashlib,json,subprocess,tempfile
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--browser',action='store_true');parser.add_argument('--assets',type=Path);parser.add_argument('--optimize-assets',action='store_true',help='Privately losslessly optimize the original corpus before decoding');args=parser.parse_args();assert not(args.assets and args.optimize_assets),'Choose prepared or newly optimized assets'
project=Path(__file__).resolve().parents[1];rows=[]
for path in sorted((project/'ports/arkanoo/assets').glob('*.png')):
 with Image.open(path) as image:rows.append({'asset':path.name,'rgba_sha256':hashlib.sha256(image.convert('RGBA').tobytes()).hexdigest()})
assert len(rows)==76,'Original image corpus changed; review the independent reference coverage'
with tempfile.TemporaryDirectory(prefix='gamepkg-png-reference-') as temporary:
 assets=args.assets or project/'ports/arkanoo/assets'
 if args.optimize_assets:
  assets=Path(temporary)/'optimized';assets.mkdir()
  for row in rows:
   target=assets/row['asset']
   with Image.open(project/'ports/arkanoo/assets'/row['asset']) as image:image.save(target,format='PNG',optimize=True)
   subprocess.run(['optipng','-quiet','-o2',str(target)],check=True,timeout=60)
 report=Path(temporary)/'reference.json';report.write_text(json.dumps({'images':rows}))
 subprocess.run(['node',str(project/'tests/web_png_exact.mjs'),str(report),str(assets)],cwd=project,check=True,timeout=30)
 if args.browser:subprocess.run(['/usr/bin/python3',str(project/'tests/browser_png_exact.py'),str(report),'--assets',str(assets)],cwd=project,check=True,timeout=60)
