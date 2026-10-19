"""Maintainer-only native/JavaScript decoder comparison; no packages or recordings published."""
from pathlib import Path
import argparse,json,os,subprocess,tempfile,sys
parser=argparse.ArgumentParser()
parser.add_argument('music',type=Path)
parser.add_argument('--browser',action='store_true')
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
decoder=project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'
build=subprocess.run(['cargo','build','--locked','--release','--target','wasm32-unknown-unknown','--manifest-path','tools/audio-decoder/Cargo.toml'],cwd=project,env=dict(os.environ,GAMEPKG_AUDIO_MAX_MEMORY_BYTES='6291456'),capture_output=True,text=True,timeout=300)
assert build.returncode==0,build.stderr
build=subprocess.run(['cargo','test','--locked','--release','--test','browser_audio_decoder','--no-run','--message-format=json'],cwd=project,capture_output=True,text=True,timeout=300)
assert build.returncode==0,build.stderr
records=[json.loads(line) for line in build.stdout.splitlines() if line.startswith('{')]
executable=next(record['executable'] for record in records if record.get('target',{}).get('name')=='browser_audio_decoder' and record.get('executable'))
with tempfile.TemporaryDirectory(prefix='gamepkg-browser-decoder-') as temporary:
    root=Path(temporary)
    environment=dict(os.environ,GAMEPKG_BROWSER_DECODER_INPUT=str(args.music.resolve()),GAMEPKG_BROWSER_DECODER_FIXTURE=str(root),GAMEPKG_AUDIO_DECODER_WASM=str(decoder))
    subprocess.run([executable,'--ignored','--nocapture'],cwd=project,env=environment,check=True,timeout=120)
    subprocess.run(['node','tests/web_recorded_decoder.mjs',str(root),str(args.music.resolve()),str(decoder)],cwd=project,check=True,timeout=120)
    if args.browser:subprocess.run([sys.executable,'tests/browser_recorded_decoder.py',str(root),str(args.music.resolve()),str(decoder)],cwd=project,check=True,timeout=60)
