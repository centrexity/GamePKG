"""Maintainer-only PNG pixel parity; no generated fixtures enter the repository."""
from pathlib import Path
import argparse,hashlib,json,subprocess,tempfile
from PIL import Image
parser=argparse.ArgumentParser();parser.add_argument('--browser',action='store_true');parser.add_argument('--assets',type=Path);args=parser.parse_args()
project=Path(__file__).resolve().parents[1];rows=[]
for path in sorted((project/'ports/arkanoo/assets').glob('*.png')):
 with Image.open(path) as image:rows.append({'asset':path.name,'rgba_sha256':hashlib.sha256(image.convert('RGBA').tobytes()).hexdigest()})
assert len(rows)==76,'Original image corpus changed; review the independent reference coverage'
with tempfile.TemporaryDirectory(prefix='gamepkg-png-reference-') as temporary:
 report=Path(temporary)/'reference.json';report.write_text(json.dumps({'images':rows}))
 subprocess.run(['node',str(project/'tests/web_png_exact.mjs'),str(report),str(args.assets or project/'ports/arkanoo/assets')],cwd=project,check=True,timeout=30)
 if args.browser:subprocess.run(['/usr/bin/python3',str(project/'tests/browser_png_exact.py'),str(report),'--assets',str(args.assets or project/'ports/arkanoo/assets')],cwd=project,check=True,timeout=60)
