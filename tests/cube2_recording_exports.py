"""Installed signed identity -> validated recordings -> explicit new output file."""
from pathlib import Path
import gzip,fcntl,hashlib,json,os,struct,subprocess,tempfile,tomllib,zipfile
PROJECT=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-recording-exports-') as temporary:
    root=Path(temporary);library=root/'library';key=root/'test.key';key.write_text('69'*32)
    config=root/'settings.toml';config.write_text('automatic_update_checks=false\n')
    env=dict(os.environ,GAMEPKG_HOME=str(library),GAMEPKG_CONFIG=str(config))
    def cli(*args,good=True):
        result=subprocess.run([str(PROJECT/'target/release/gamepkg'),*map(str,args)],env=env,capture_output=True,text=True,timeout=30)
        assert (result.returncode==0)==good,(args,result.stdout,result.stderr)
        return result.stdout
    package=root/'test.gamepkg';cli('pack',PROJECT/'ports/cube2','-o',package,'--key',key)
    cli('install',package,'--trust-creator')
    assert json.loads(cli('recordings','com.gamepkg.cube2','--json'))==[]
    with zipfile.ZipFile(package) as archive:manifest=tomllib.loads(archive.read('manifest.toml').decode())
    scope=f"GamePKG Cube 2 saved files v1\0{manifest['id']}\0{manifest['creator_key']}\0{manifest['save_schema']}"
    save=library/'saves'/(hashlib.sha256(scope.encode()).hexdigest()+'.cube2')
    def bundle(records):
        payload=b''
        for name,data in sorted(records):
            encoded=name.encode();payload+=struct.pack('<4I',len(encoded),len(data),0,0)+encoded+data
        return b'GPS2'+struct.pack('<3I',len(records),len(payload),0)+payload
    movie=b'RIFF private movie fixture\x00\xff';demo=b'private demo fixture\x00\xfe'
    original=bundle([('Movie.AVI',movie),('autoexec.cfg',b'private startup commands'),('config.cfg',b'private preferences'),('recordings/one.dMo',demo)])
    incoming=root/'incoming.dmo';incoming.write_bytes(gzip.compress(b'SAUERBRATEN_DEMO'+struct.pack('<5I',1,260,0,1,2)+b'\0\1'))
    save.write_bytes(original)
    with save.with_name(save.name+'.writer').open('a+b') as lease:
        fcntl.flock(lease,fcntl.LOCK_EX|fcntl.LOCK_NB)
        entries=json.loads(cli('recordings','com.gamepkg.cube2','--json'))
        assert entries==[dict(name='Movie.AVI',bytes=len(movie),kind='movie'),dict(name='recordings/one.dMo',bytes=len(demo),kind='demo')],entries
        output=root/'export.avi';cli('recording-export','com.gamepkg.cube2','Movie.AVI','--output',output)
        assert output.read_bytes()==movie and save.read_bytes()==original
        cli('recording-import','com.gamepkg.cube2',incoming,good=False)
        assert save.read_bytes()==original
    cli('recording-export','com.gamepkg.cube2','recordings/one.dMo','-o',output,good=False)
    assert output.read_bytes()==movie
    linked=root/'linked.avi';linked.symlink_to(output)
    cli('recording-export','com.gamepkg.cube2','Movie.AVI','-o',linked,good=False)
    assert output.read_bytes()==movie
    for name in ['autoexec.cfg','config.cfg','../Movie.AVI','missing.avi','/Movie.AVI']:
        destination=root/'denied.avi'
        cli('recording-export','com.gamepkg.cube2',name,'-o',destination,good=False)
        assert not destination.exists()
    cli('recording-import','com.gamepkg.cube2',incoming)
    imported=save.read_bytes()
    assert imported==bundle([('Movie.AVI',movie),('autoexec.cfg',b'private startup commands'),('config.cfg',b'private preferences'),('recordings/one.dMo',demo),('demo/incoming.dmo',incoming.read_bytes())])
    incoming_export=root/'imported.dmo';cli('recording-export','com.gamepkg.cube2','demo/incoming.dmo','-o',incoming_export)
    assert incoming_export.read_bytes()==incoming.read_bytes()
    cli('recording-import','com.gamepkg.cube2',incoming,good=False)
    assert save.read_bytes()==imported
    for name in ['../escape.dmo','autoexec.cfg','config.cfg','/escape.dmo']:
        cli('recording-import','com.gamepkg.cube2',incoming,'--name',name,good=False)
        assert save.read_bytes()==imported
    bad=root/'damaged.dmo';data=bytearray(incoming.read_bytes());data[-1]^=1;bad.write_bytes(data)
    cli('recording-import','com.gamepkg.cube2',bad,good=False);assert save.read_bytes()==imported
    large=root/'oversize.dmo'
    with large.open('wb') as target:target.truncate(16*1048576+1)
    cli('recording-import','com.gamepkg.cube2',large,good=False);assert save.read_bytes()==imported
    saturated=bundle([(f'demo/{i:03}.dmo',b'kept') for i in range(256)]);save.write_bytes(saturated)
    cli('recording-import','com.gamepkg.cube2',incoming,good=False);assert save.read_bytes()==saturated
    save.write_bytes(original)
    save.write_bytes(bundle([('../escape.avi',b'bad')]))
    cli('recordings','com.gamepkg.cube2',good=False)
    cli('recording-export','com.gamepkg.cube2','../escape.avi','-o',root/'denied.avi',good=False)
    assert not (root/'denied.avi').exists()
    save.write_bytes(original)
    save.rename(root/'private-save');os.link(root/'private-save',save)
    cli('recordings','com.gamepkg.cube2',good=False)
    cli('recording-export','com.gamepkg.cube2','Movie.AVI','-o',root/'linked-leak.avi',good=False)
    cli('recording-import','com.gamepkg.cube2',incoming,good=False)
    assert not (root/'linked-leak.avi').exists() and (root/'private-save').read_bytes()==original
    save.unlink();save.symlink_to(root/'private-save')
    cli('recordings','com.gamepkg.cube2',good=False)
    assert (root/'private-save').read_bytes()==original
print('Signed recording commands: scoped listing, exact export, preserved source, no overwrite/followed links, private-settings exclusion, missing/traversal/corrupt-save rejection, active-writer import denial, exact import/export and duplicate/quota/damaged-input preservation passed.')
