"""Malformed/complex Rust must fail inspection cleanly, never abort the host."""
from pathlib import Path
import os, shutil, subprocess, tempfile
PROJECT=Path(__file__).resolve().parents[1];CLI=PROJECT/'target/release/gamepkg'
with tempfile.TemporaryDirectory(prefix='gamepkg-parser-limits-') as temporary:
    root=Path(temporary);source=root/'source';shutil.copytree(PROJECT/'examples/minimal',source)
    key=root/'key';key.write_text('75'*32)
    cases=[
        'let ignored = '+'!'*10000+'false;',
        'let ignored = '+'('*1000+'0'+')'*1000+';',
        'let ignored = '+'1 + '*2000+'0;',
    ]
    for index,body in enumerate(cases):
        (source/'source/src/lib.rs').write_text('pub fn game_tick(_:u32,state:u64)->u64 {'+body+' state }')
        output=root/f'rejected-{index}.gamepkg'
        result=subprocess.run([str(CLI),'pack',str(source),'-o',str(output),'--key',str(key)],env=dict(os.environ,GAMEPKG_CONFIG=str(root/'settings.toml')),capture_output=True,text=True,timeout=10)
        assert result.returncode==1,(index,result.returncode,result.stderr)
        assert 'budget' in result.stderr and 'stack overflow' not in result.stderr,(index,result.stderr)
        assert not output.exists() and not output.with_suffix('.gamepkg.partial').exists()
    print('Parser limits passed: excessive nesting/operators fail cleanly before AST parsing and publish no package.')
