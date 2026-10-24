"""Maintainer-only original SPC RAM-map byte order proof, without ROM data."""
from pathlib import Path
import argparse
import json
import os
import re
import resource
import signal
import subprocess
import tempfile

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cc',default='cc')
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--big-endian',action='store_true',help='qualify the maintained 32-bit PowerPC/QEMU cross-toolchain')
parser.add_argument('--emulator',type=Path)
parser.add_argument('--sysroot',type=Path)
parser.add_argument('--compiler-prefix',type=Path)
parser.add_argument('--playback',action='store_true',help='exercise the complete live player/DSP with synthetic sound banks')
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
upstream=project/'engines/zelda3/upstream'
source=(upstream/'src/spc_player.c').read_text()
unmodified=source
def function(text,name):
    definitions=list(re.finditer(r'\bvoid\s+'+re.escape(name)+r'\([^;{]*\)\s*\{',text))
    assert len(definitions)==1
    start=definitions[0].start();brace=text.index('{',start);end=brace+1;depth=1
    while depth:
        depth+=(text[end]=='{')-(text[end]=='}');end+=1
    return text[start:end]
maps=source[source.index('typedef struct MemMap {'):source.index('static void PlayNote(')]
original=function(source,'SpcPlayer_CopyVariablesToRam').replace('SpcPlayer_CopyVariablesToRam','reference_to')+'\n'
original+=function(source,'SpcPlayer_CopyVariablesFromRam').replace('SpcPlayer_CopyVariablesFromRam','reference_from')+'\n'
for patch in json.loads((project/'engines/zelda3/adapters/spc-state-patches.json').read_text()):
    assert source.count(patch['before'])==patch['count']==1
    source=source.replace(patch['before'],patch['after'])
body=maps+original+function(source,'SpcPlayer_CopyVariablesToRam')+'\n'+function(source,'SpcPlayer_CopyVariablesFromRam')+'\n'
# Every two-byte entry must actually be a native uint16, not a byte array.
for field,offset in re.findall(r'offsetof\(Channel, (\w+)\), (0x[0-9a-f]+)',maps):
    ctype='uint16_t *' if int(offset,16)&0x8000 else 'uint8_t *'
    body+=f'_Static_assert(_Generic(&((Channel*)0)->{field},{ctype}:1,default:0),"Channel map type: {field}");\n'
for field,size in re.findall(r'offsetof\(SpcPlayer, (\w+)\), 0x[0-9a-f]+, ([124])',maps):
    ctype={'1':'uint8_t *','2':'uint16_t *','4':'uint8_t (*)[4]'}[size]
    body+=f'_Static_assert(_Generic(&((SpcPlayer*)0)->{field},{ctype}:1,default:0),"SPC map type: {field}");\n'
with tempfile.TemporaryDirectory(prefix='gamepkg-spc-state-') as temporary:
    root=Path(temporary);helpers=root/'helpers.h';helpers.write_text(body);binary=root/'test'
    fixture=project/'tests/zelda_spc_state.c';extra=[]
    if args.playback:
        for patch in json.loads((project/'engines/zelda3/adapters/spc-playback-patches.json').read_text()):
            assert source.count(patch['before'])==patch['count']
            source=source.replace(patch['before'],patch['after'])
        player=root/'spc_player.c';player.write_text(source)
        references=''
        for name,renamed in [('SpcPlayer_Upload','reference_upload'),('Music_ResetChan','reference_reset'),('Port0_HandleMusic','reference_music'),('Spc_Loop_Part2','reference_tick')]:
            references+=function(unmodified,name).replace(name,renamed)+'\n'
        helpers.write_text(references)
        dsp=(upstream/'snes/dsp.c').read_text()
        legacy_brr=function(dsp,'dsp_decodeBrr').replace('dsp_decodeBrr','gamepkg_legacy_brr')
        for patch in json.loads((project/'engines/zelda3/adapters/dsp-state-patches.json').read_text()):
            assert dsp.count(patch['before'])==patch['count']==1
            dsp=dsp.replace(patch['before'],patch['after'])
        dsp_source=root/'dsp.c';dsp_source.write_text(dsp+'\n'+legacy_brr+'\nvoid gamepkg_test_decode_brr(Dsp *dsp,int ch) { dsp_decodeBrr(dsp,ch); }\n');extra=[str(dsp_source)]
        fixture=project/'tests/zelda_spc_playback.c'
    flags=['-std=c11','-O1','-g','-Wall','-Wextra','-Werror']
    compiler_environment=dict(os.environ)
    if args.big_endian:
        assert args.emulator and args.sysroot and args.compiler_prefix
        flags+=['-DGAMEPKG_EXPECT_BIG_ENDIAN','-DGAMEPKG_TEST_32','--sysroot='+str(args.compiler_prefix),'-B'+str(args.compiler_prefix/'usr/powerpc-linux-gnu/lib')+'/', '-isystem',str(args.compiler_prefix/'usr/powerpc-linux-gnu/include'),'-L'+str(args.compiler_prefix/'usr/powerpc-linux-gnu/lib'),'-fsanitize=undefined','-fsanitize-undefined-trap-on-error']
        compiler_environment['LD_LIBRARY_PATH']=str(args.compiler_prefix/'usr/lib/x86_64-linux-gnu')+(':'+os.environ['LD_LIBRARY_PATH'] if os.environ.get('LD_LIBRARY_PATH') else '')
    elif args.bits32:
        flags+=['-m32','-DGAMEPKG_TEST_32','-isystem','/usr/include/x86_64-linux-gnu','-fsanitize=undefined','-fsanitize-undefined-trap-on-error']
    else:
        flags+=['-fsanitize=address,undefined']
    if args.playback:flags+=[f'-DGAMEPKG_SPC_PLAYER_SOURCE="{player}"']
    subprocess.run([args.cc,*flags,'-I'+str(upstream),'-iquote',str(upstream/'src'),'-iquote',str(upstream/'snes'),'-I'+str(project/'engines/zelda3/adapters/include'),f'-DGAMEPKG_SPC_HELPERS="{helpers}"',str(fixture),*extra,'-o',str(binary)],check=True,env=compiler_environment)
    elf=binary.read_bytes()[:6]
    assert elf[:4]==b'\x7fELF'
    if args.bits32 or args.big_endian:assert elf[4]==1
    if args.big_endian:assert elf[5]==2,'Not an actual big-endian executable'
    command=([str(args.emulator),'-L',str(args.sysroot)] if args.emulator else [])+[str(binary)]
    subprocess.run(command,env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1'),check=True,timeout=180)
    if args.playback:
        def no_core():resource.setrlimit(resource.RLIMIT_CORE,(0,0))
        for option,marker,diagnostic in [('--legacy-unaligned-upload','LEGACY_UNALIGNED_UPLOAD','misaligned address'),('--legacy-brr','LEGACY_NEGATIVE_BRR','left shift of negative value')]:
            red=subprocess.run(command+[option],env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1'),capture_output=True,text=True,timeout=30,preexec_fn=no_core)
            assert marker in red.stdout,red.stdout+red.stderr
            assert diagnostic in red.stderr or red.returncode in [-signal.SIGILL,-signal.SIGTRAP],red.stdout+red.stderr
            assert red.returncode!=0,'Original undefined operation unexpectedly passed'
        print('Original unaligned upload and negative BRR shifts are rejected by the unchanged undefined-behavior sanitizer gate.')
