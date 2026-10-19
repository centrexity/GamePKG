"""Maintainer-only original audio control oracle; no recordings or game packages produced."""
from pathlib import Path
import argparse, hashlib, os, subprocess, tempfile
parser=argparse.ArgumentParser()
parser.add_argument('--original',type=Path,required=True)
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
source=(args.original/'src/audio.rs').resolve()
assert hashlib.sha256(source.read_bytes()).hexdigest()=='aa2767daa92576fa0fc093e4f058d4eafe322305e6e31a4bc9daf987bb8a92b2', 'Expected the pinned original audio source unchanged'
with tempfile.TemporaryDirectory(prefix='gamepkg-original-music-') as temporary:
    root=Path(temporary)
    (root/'assets/music').mkdir(parents=True)
    for name in ['z.mp3','a.mp3','m.mp3','ignored.MP3','ignored.txt']:(root/'assets/music'/name).write_bytes(b'private control fixture')
    environment=dict(os.environ,GAMEPKG_ORIGINAL_AUDIO_RS=str(source),GAMEPKG_ORIGINAL_MUSIC_FIXTURE_HOME=str(root))
    compiled=subprocess.run(['rustc','--edition=2021',str(project/'tests/fixtures/original-music-controls.rs'),'-o',str(root/'controls')],env=environment,capture_output=True,text=True,timeout=60)
    assert compiled.returncode==0, compiled.stdout+'\n'+compiled.stderr
    subprocess.run([str(root/'controls')],cwd=root,env=environment,check=True,timeout=10)
