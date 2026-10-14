"""Check home-folder defaults and metadata survival across native host restarts."""
import argparse, json, os, subprocess, tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--indexeddb-package',type=Path,required=True);args=p.parse_args()
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-home-db-') as temporary:
    root=Path(temporary);home=root/'home';home.mkdir()
    env=dict(os.environ,HOME=str(home),USERPROFILE=str(home))
    for variable in ['GAMEPKG_HOME','GAMEPKG_CONFIG','XDG_DATA_HOME','XDG_CONFIG_HOME','APPDATA','LOCALAPPDATA']:env.pop(variable,None)
    def run(*arguments):
        result=subprocess.run([str(CLI),*map(str,arguments)],env=env,capture_output=True,text=True,timeout=30)
        assert result.returncode==0,result.stderr
        return result.stdout
    key=root/'key';key.write_text('65'*32)
    run('pack',PROJECT/'ports/2048','-o',root/'2048.gamepkg','--key',key)
    run('install',root/'2048.gamepkg','--trust-creator')
    library=home/'.local/share/gamepkg';database=library/'database/library.json'
    assert (library/'com.gamepkg.2048.gamepkg').is_file()
    previous=None
    for _ in range(2):
        host=subprocess.Popen([str(CLI),'web-library','--indexeddb-package',str(args.indexeddb_package),'--port','0'],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            line=host.stdout.readline();assert line.startswith('Open http://'),line
            record=json.loads(database.read_text());assert record['games'][0]['manifest']['id']=='com.gamepkg.2048'
            if previous is not None:
                for field in ["scope","games","catalog","settings","rejected"]:assert record[field]==previous[field],field
            previous=record
        finally:
            host.terminate();host.wait(timeout=10)
        assert 'com.gamepkg.2048' in run('list')
    assert database.is_file()
    print('Home database passed: default home folder, native restart, library identity and persistent metadata.')
