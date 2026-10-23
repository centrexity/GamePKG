"""Prepare a PRIVATE full-port recorded-audio candidate; never modifies published assets.
Maintainer Python only. All music/effects remain package-owned data.
"""
from pathlib import Path
import argparse, hashlib, json, shutil
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--private-output',type=Path,required=True)
parser.add_argument('--migration-inputs',type=Path,required=True)
args=parser.parse_args();project=Path(__file__).resolve().parents[1];out=args.private_output.resolve()
assert project not in out.parents and out!=project and not out.exists(),'Require new private output outside workspace'
inputs=args.migration_inputs;metadata=json.loads((inputs/'migration-inputs.json').read_text())
assert len(metadata['effects'])==5 and len(metadata['music'])==9,'Require complete original dataset'
paths=[item['asset'] for item in metadata['effects']+metadata['music']]
assert len(set(paths))==14,'Duplicate recording path'
for name in paths:
 assert isinstance(name,str) and not Path(name).is_absolute() and '..' not in Path(name).parts and '\\' not in name,'Invalid recording path'
 assert name.startswith(('audio/','music/')) and len(Path(name).parts)==2,'Invalid recording directory'
assert (inputs/'audio/music-list.txt').read_text()==''.join(item['asset']+'\n' for item in metadata['music']),'Playlist must retain every track in order'
for item in metadata['effects']+metadata['music']:
 path=inputs/item['asset'];assert not path.is_symlink() and path.is_file()
 expected=item.get('wav_sha256',item.get('source_sha256'))
 assert hashlib.sha256(path.read_bytes()).hexdigest()==expected,path.name
shutil.copytree(project/'ports/arkanoo',out)
for item in metadata['effects']+metadata['music']:
 target=out/'assets'/item['asset'];target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(inputs/item['asset'],target)
shutil.copy2(inputs/'audio/music-list.txt',out/'assets/audio/music-list.txt')
# Game/control/runtime integration lives in the canonical source. Assembly only
# supplies the complete matched assets and signed shares. The playlist selects
# the recorded profile at startup, without source switches or implementations.
manifest=out/'manifest.toml';text=manifest.read_text();assert 'effects_memory_bytes' not in text and 'music_memory_bytes' not in text
manifest.write_text(text.replace('recommended_memory_bytes = 8388608','recommended_memory_bytes = 16777216').replace('[limits]','[limits]\neffects_memory_bytes = 3145728\nmusic_memory_bytes = 7340032'))
for source in (project/'ports/arkanoo/source/src').glob('*.rs'):
 expected=source.read_bytes()
 assert (out/'source/src'/source.name).read_bytes()==expected,'Assembly must not patch game implementations: '+source.name
print('Private complete recorded-audio port candidate:',out)
print('Identical canonical source; only signed shares and verified assets differ.')
print('All five original effects and nine music tracks retained; existing hard memory limit unchanged.')
