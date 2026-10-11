"""Compile both profiles, verify indexed ROMs, detect tampering, repair from signed source."""
import os, shutil, subprocess, tempfile
from pathlib import Path
PROJECT=Path(__file__).resolve().parents[1]
CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-integrity-roms-') as directory:
    root=Path(directory)
    for profile in ['core-v1','component-v1']:
        home=root/profile; source=home/'source'
        shutil.copytree(PROJECT/'examples/minimal',source)
        manifest=source/'manifest.toml'
        manifest.write_text(f'runtime_profile="{profile}"\n'+manifest.read_text()+'\n[[roms]]\nid="program"\ntitle="Program ROM"\n[[roms]]\nid="data"\ntitle="Data ROM"\n')
        (source/'source/src/lib.rs').write_text('''pub fn game_tick(_:u32,state:u64)->u64 {
            assert_eq!(gamepkg::rom_count(),2);
            assert_eq!(gamepkg::rom_size_at(0),Some(3));
            assert_eq!(gamepkg::rom_size_at(1),Some(4));
            assert_eq!(gamepkg::rom_size_at(9999),None);
            let mut bytes=[0u8;4];
            assert_eq!(gamepkg::rom_read_at(1,0,&mut bytes),Some(4));
            assert_eq!(&bytes,b"DATA");
            assert_eq!(gamepkg::rom_read_at(9999,0,&mut bytes),None);
            assert_eq!(gamepkg::rom_size(),Some(3)); state+1
        }''')
        env=dict(os.environ,GAMEPKG_HOME=str(home/'library'),GAMEPKG_CONFIG=str(home/'settings.toml'))
        def run(*args,good=True):
            result=subprocess.run([str(CLI),*map(str,args)],env=env,capture_output=True,text=True,timeout=180)
            assert (result.returncode==0)==good,result.stdout+result.stderr
            return result.stdout+result.stderr
        key=home/'creator.key';run('keygen','-o',key)
        package=home/'game.gamepkg';run('pack',source,'-o',package,'--key',key)
        run('install',package,'--trust-creator');run('build','com.gamepkg.minimal')
        a=home/'program.rom';a.write_bytes(b'ROM');b=home/'data.rom';b.write_bytes(b'DATA')
        run('run','com.gamepkg.minimal','--frames','1','--rom',a,good=False)
        run('run','com.gamepkg.minimal','--frames','1','--rom',a,'--rom',b)
        run('integrity-check','com.gamepkg.minimal')
        build=next((home/'library/builds').glob('*/game.wasm'));original=build.read_bytes()
        build.write_bytes(original+b'damaged')
        run('integrity-check','com.gamepkg.minimal',good=False)
        run('run','com.gamepkg.minimal','--frames','1','--rom',a,'--rom',b,good=False)
        run('integrity-fix','com.gamepkg.minimal')
        run('run','com.gamepkg.minimal','--frames','1','--rom',a,'--rom',b)
        installed=home/'library/com.gamepkg.minimal.gamepkg';installed.write_bytes(b'damaged package')
        run('integrity-fix','com.gamepkg.minimal')
        run('integrity-check','com.gamepkg.minimal')
        installed.write_bytes(b'damaged package')
        backup=home/'library/verified-packages/com.gamepkg.minimal.gamepkg'
        backup.write_bytes(b'damaged recovery copy')
        before=build.read_bytes()
        run('integrity-fix','com.gamepkg.minimal',good=False)
        assert installed.read_bytes()==b'damaged package' and build.read_bytes()==before
        print(profile+': multiple ROMs, missing ROM rejection, integrity detection, verified rebuild and damaged-source refusal passed')
