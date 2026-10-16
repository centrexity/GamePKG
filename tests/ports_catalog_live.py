"""Opt-in live release check: repository URL -> catalog -> signed port downloads."""
from pathlib import Path
import hashlib,json,os,subprocess,tempfile,tomllib,zipfile
PROJECT=Path(__file__).resolve().parents[1]
CLI=Path(os.environ.get('GAMEPKG_TEST_BINARY_DIR',PROJECT/'target/release'))/'gamepkg'
REPO='https://github.com/centrexity/GamePKG_Ports'
with tempfile.TemporaryDirectory(prefix='gamepkg-live-port-catalog-') as temp:
 root=Path(temp);home=root/'library';config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
 env=dict(os.environ,GAMEPKG_HOME=str(home),GAMEPKG_CONFIG=str(config))
 def run(*args,good=True):
  r=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=120)
  assert (r.returncode==0)==good,r.stdout+r.stderr
  return r.stdout+r.stderr
 assert '4 catalog games' in run('import-list',REPO)
 catalog=tomllib.loads((home/'catalog.gamepkglist').read_text());assert len(catalog['games'])==4
 assert not list(home.glob('*.gamepkg')) and not (home/'builds').exists(),'Import downloaded game content'
 assert run('list').count('Not downloaded')==4
 expected=json.loads((PROJECT/'ports/creator-identity.json').read_text())
 run('download',catalog['games'][0]['id'],good=False)
 assert not list(home.glob('*.gamepkg')),'Download installed without creator approval'
 for game in catalog['games']:
  run('download',game['id'],'--trust-creator')
  package=home/(game['id']+'.gamepkg');run('verify',package)
  with zipfile.ZipFile(package) as z:
   m=tomllib.loads(z.read('manifest.toml').decode())
   assert m['creator_key']==expected['creator_public_key']
   assert m['version']==game['version'] and m['id']==game['id']
   assert m['update_url'].startswith(REPO+'/releases/latest/download/')
   assert not any(n.lower().endswith(('.key','.wad','.sfc','.smc','.rom')) for n in z.namelist())
  assert 'up to date' in run('check-updates',game['id']).lower()
 assert not (home/'builds').exists(),'Downloads built or ran games'
 env['GAMEPKG_HOME']=str(root/'upgrade-library')
 run('install',PROJECT/'html/demos/zelda3.gamepkg','--trust-creator')
 run('import-list',REPO)
 run('download','com.gamepkg.zelda3')
 with zipfile.ZipFile(root/'upgrade-library/com.gamepkg.zelda3.gamepkg') as z:
  upgraded=tomllib.loads(z.read('manifest.toml').decode())
  assert upgraded['creator_key']==expected['creator_public_key'] and upgraded['version']=='0.1.2'
 print('Live ports import passed: repository URL, four Not downloaded entries, creator approval, all four published signatures, matching versions, no commercial assets and working latest-release update URLs.')
