"""Maintainer-only original-rate browser mixing/conversion qualification."""
from pathlib import Path
import argparse,subprocess,sys
parser=argparse.ArgumentParser();parser.add_argument('--browser',action='store_true');args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
result=subprocess.run(['cargo','build','--locked','--release','--target','wasm32-unknown-unknown','--manifest-path','tools/audio-converter/Cargo.toml'],cwd=project,capture_output=True,text=True,timeout=300)
assert result.returncode==0,result.stderr
wasm=project/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'
subprocess.run(['node','tests/web_recorded_mixer.mjs',str(wasm)],cwd=project,check=True,timeout=30)
if args.browser:subprocess.run([sys.executable,'tests/browser_recorded_mixer.py',str(wasm)],cwd=project,check=True,timeout=60)
