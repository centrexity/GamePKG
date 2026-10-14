"""Run the actual Windows host with a local ROM; no remote ROM transport."""
from pathlib import Path
import argparse,os,subprocess
PROJECT=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('rom',type=Path);p.add_argument('--prefix',type=Path,required=True);p.add_argument('--python',type=Path,required=True);args=p.parse_args()
root=PROJECT/'target/zelda3-windows-play';root.mkdir(parents=True,exist_ok=True)
config=root/'settings.toml';config.write_text('automatic_update_checks=false\nfuel_ceiling=100000000\n')
def windows(path):return 'Z:'+str(Path(path).resolve()).replace('/','\\')
env=dict(os.environ,WINEPREFIX=str(args.prefix.resolve()),WINEDEBUG='-all',GAMEPKG_HOME=windows(root/'library'),GAMEPKG_CONFIG=windows(config),GAMEPKG_ZELDA3_PYTHON=windows(args.python))
exe=PROJECT/'target/x86_64-pc-windows-gnu/release/gamepkg.exe'
def run(*args):
 result=subprocess.run(['wine',str(exe),*map(str,args)],env=env,capture_output=True,text=True,timeout=180)
 assert result.returncode==0,result.stdout+result.stderr
 print(result.stdout.strip(),flush=True)
run('store-import',windows(PROJECT/'target/zelda3-exchange/request.gamepkgstore'),'--trust-creators')
run('accept-build-worker','com.gamepkg.zelda3',windows(PROJECT/'target/zelda3-exchange/response'))
run('integrity-check','com.gamepkg.zelda3')
run('run','com.gamepkg.zelda3','--rom',windows(args.rom),'--frames',1000)
print('Windows executable under Wine: verified engine exchange, selected-ROM in-memory decoding, 1000 frames and audio passed.')
