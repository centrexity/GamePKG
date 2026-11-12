"""Development-only pinned Zelda3 Wasm build; no arbitrary package C accepted."""
from pathlib import Path
import argparse, hashlib, json, shutil, subprocess
PROJECT=Path(__file__).resolve().parents[1]
ENGINE=PROJECT/'engines/zelda3';UPSTREAM=ENGINE/'upstream'
p=argparse.ArgumentParser();p.add_argument('--sysroot',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
lock=json.loads((ENGINE/'upstream-lock.json').read_text())
for name,digest in lock['files'].items():
    path=UPSTREAM/name
    if path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest()!=digest:raise SystemExit('Pinned Zelda3 source changed: '+name)
args.output.parent.mkdir(parents=True,exist_ok=True)
work=args.output.parent/'zelda3-stage';shutil.copytree(UPSTREAM,work,dirs_exist_ok=True)
# Save IO is supplied by the adapter; desktop replay/debug/IO code is dead stripped.
def remove_function(text,name):
    import re
    match=re.search(r'\n(?:NORETURN )?(?:bool|void) '+name+r'\([^;]*?\)\s*\{',text)
    if not match:raise ValueError('Missing upstream function '+name)
    start=match.start();i=match.end();depth=1
    while depth:
        depth+=(text[i]=='{')-(text[i]=='}');i+=1
    return text[:start]+text[i:]
rtl=work/'src/zelda_rtl.c';text=rtl.read_text()
for name in ['ZeldaReadSram','ZeldaWriteSram']:text=remove_function(text,name)
for anchor in ['static void InternalSaveLoad(', 'static void SaveSnesState(', 'static void LoadSnesState(']:
    assert text.count(anchor)==1,'Pinned snapshot helper changed'
assert 'gamepkg_snapshot_bridge.h' not in text
rtl.write_text(text+'\n#include "gamepkg_snapshot_bridge.h"\n')
patch=json.loads((ENGINE/'adapters/ppu-patches.json').read_text())
assert patch['file']=='snes/ppu.c' and patch['count']==3
ppu=work/patch['file'];text=ppu.read_text()
for item,count in [(patch,3),(patch['unsigned_masks'],1),(patch['mosaic_spans'][0],2),(patch['mosaic_spans'][1],1)]:
    assert item['count']==count and text.count(item['before'])==count,'Pinned PPU adaptation anchor changed'
    text=text.replace(item['before'],item['after'])
ppu.write_text(text)
for patch in json.loads((ENGINE/'adapters/ppu-state-patches.json').read_text()):
    assert text.count(patch['before'])==patch['count']==1,'Pinned PPU state anchor changed'
    text=text.replace(patch['before'],patch['after'])
ppu.write_text(text)
dma=work/'snes/dma.c';text=dma.read_text()
for patch in json.loads((ENGINE/'adapters/dma-state-patches.json').read_text()):
    assert text.count(patch['before'])==patch['count']==1,'Pinned DMA state anchor changed'
    text=text.replace(patch['before'],patch['after'])
dma.write_text(text)
dsp=work/'snes/dsp.c';text=dsp.read_text()
for patch in json.loads((ENGINE/'adapters/dsp-state-patches.json').read_text()):
    assert text.count(patch['before'])==patch['count']==1,'Pinned DSP state anchor changed'
    text=text.replace(patch['before'],patch['after'])
dsp.write_text(text)
spc=work/'src/spc_player.c';text=spc.read_text()
for patch in json.loads((ENGINE/'adapters/spc-state-patches.json').read_text()):
    assert text.count(patch['before'])==patch['count']==1,'Pinned SPC state anchor changed'
    text=text.replace(patch['before'],patch['after'])
spc.write_text(text)
for patch in json.loads((ENGINE/'adapters/spc-playback-patches.json').read_text()):
    assert text.count(patch['before'])==patch['count'],'Pinned SPC playback anchor changed'
    text=text.replace(patch['before'],patch['after'])
spc.write_text(text)


skip={'main.c','audio.c','config.c','glsl_shader.c','opengl.c','zelda_cpu_infra.c'}
sources=[path for path in sorted((work/'src').glob('*.c')) if path.name not in skip]
sources += [work/'snes'/name for name in ['ppu.c','dma.c','dsp.c','snes_other.c']]
sources += [ENGINE/'adapters/engine.c']
clang='/usr/bin/clang-18'
prefix=args.sysroot.resolve()
if (prefix/'usr/include/wasm32-wasi').is_dir():prefix=prefix/'usr'
resource=prefix/'lib/llvm-18/lib/clang/18'
flags=['--target=wasm32-wasi','--sysroot='+str(prefix),'-resource-dir='+str(resource),'-isystem','/usr/lib/llvm-18/lib/clang/18/include','-O2','-mbulk-memory','-fno-strict-aliasing','-ffunction-sections','-fdata-sections','-I'+str(work),'-I'+str(ENGINE/'adapters/include')]
for source in sources:
    subprocess.run([clang,*flags,'-fsyntax-only',str(source)],check=True)
subprocess.run([clang,*flags,*map(str,sources),'-nostartfiles','-Wl,--no-entry','-Wl,--export-memory','-Wl,--gc-sections','-Wl,--wrap=memcpy','-Wl,--wrap=memmove','-Wl,--wrap=memset',*[f'-Wl,--wrap=__wasi_{name}' for name in ['fd_close','fd_fdstat_get','fd_seek','fd_write']],'-Wl,--initial-memory=4194304','-Wl,--max-memory=16777216','-Wl,-z,stack-size=1048576','-o',str(args.output)],check=True)
print('Built pinned Zelda3 Wasm engine:',args.output)
