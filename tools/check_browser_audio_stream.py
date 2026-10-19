"""Maintainer live FIFO/mixer qualification against an independent SDL prefix."""
from pathlib import Path
import argparse,subprocess,sys
parser=argparse.ArgumentParser();parser.add_argument('--browser',action='store_true');args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
subprocess.run([sys.executable,'tools/check_live_audio_reference.py'],cwd=project,check=True,timeout=30)
result=subprocess.run(['cargo','build','--locked','--release','--target','wasm32-unknown-unknown','--manifest-path','tools/audio-converter/Cargo.toml'],cwd=project,capture_output=True,text=True,timeout=300)
assert result.returncode==0,result.stderr
wasm=project/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm'
subprocess.run(['node','tests/web_recorded_budget.mjs'],cwd=project,check=True,timeout=30)
subprocess.run(['node','tests/web_recorded_wav.mjs'],cwd=project,check=True,timeout=30)
subprocess.run(['node','tests/web_recorded_queue.mjs'],cwd=project,check=True,timeout=30)
subprocess.run(['node','tests/web_recorded_transport.mjs'],cwd=project,check=True,timeout=30)
subprocess.run(['node','tests/web_recorded_stream.mjs',str(wasm)],cwd=project,check=True,timeout=30)
if args.browser:
 subprocess.run([sys.executable,'tests/browser_recorded_stream.py',str(wasm)],cwd=project,check=True,timeout=60)
 result=subprocess.run(['cargo','build','--locked','--release','--target','wasm32-unknown-unknown','--manifest-path','tools/audio-decoder/Cargo.toml'],cwd=project,capture_output=True,text=True,timeout=300)
 assert result.returncode==0,result.stderr
 decoder=project/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm'
 subprocess.run([sys.executable,'tests/browser_recorded_transport.py',str(wasm),str(decoder)],cwd=project,check=True,timeout=60)
