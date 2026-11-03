"""Compare the bounded Rust replay decoder with the pinned original C reader."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--linker',default='gcc')
args=parser.parse_args()
project=Path(__file__).resolve().parents[1]
upstream=project/'engines/zelda3/upstream/src'
original=(upstream/'zelda_rtl.c').read_text()
def extract_function(signature):
    start=original.index(signature)
    brace=original.index('{',start);end=brace+1;depth=1
    while depth:
        depth+=(original[end]=='{')-(original[end]=='}');end+=1
    return original[start:end]
start=original.index('typedef struct StateRecorder {')
end=original.index('} StateRecorder;',start)+len('} StateRecorder;')
structure=original[start:end]
reader=extract_function('uint16 StateRecorder_ReadNextReplayState(')
# The fixture admits only nonnegative positions <=65536. These comparison-only
# casts preserve original results while retaining strict signedness warnings.
anchor='if (replay_pos >= sr->log.size)'
assert reader.count(anchor)==1
reader=reader.replace(anchor,'if ((size_t)replay_pos >= sr->log.size)')
anchor='if (replay_pos != sr->replay_pos_last_complete)'
assert reader.count(anchor)==1
reader=reader.replace(anchor,'if ((uint32)replay_pos != sr->replay_pos_last_complete)')
lock=tomllib.loads((project/'Cargo.lock').read_text())
anyhow=next(p['version'] for p in lock['package'] if p['name']=='anyhow')
with tempfile.TemporaryDirectory(prefix='gamepkg-original-replay-') as temporary:
    root=Path(temporary);header=root/'original.h'
    writer='\n'.join(extract_function(name) for name in [
        'void ByteArray_AppendVl(',
        'void StateRecorder_RecordCmd(',
        'void StateRecorder_Record(',
        'void StateRecorder_RecordPatchByte(',
    ])
    header.write_text(structure+'\n'+writer+'\n'+reader+'\n')
    oracle=root/'original'
    flags=['-std=c11','-O1','-g','-Wall','-Wextra','-Werror']
    if args.bits32:flags+=['-m32','-isystem','/usr/include/x86_64-linux-gnu','-fsanitize=undefined','-fsanitize-undefined-trap-on-error']
    else:flags+=['-fsanitize=address,undefined']
    subprocess.run([args.cc,*flags,'-iquote',str(upstream),f'-DGAMEPKG_ORIGINAL_REPLAY="{header}"',str(project/'tests/zelda_replay_original.c'),'-o',str(oracle)],check=True)
    (root/'lib.rs').write_text('extern crate self as gamepkg;\n#[path='+json.dumps(str(project/'src/zelda_replay.rs'))+']\npub mod zelda_replay;\n#[path='+json.dumps(str(project/'tests/zelda_replay.rs'))+']\nmod tests;\n')
    (root/'Cargo.toml').write_text('[package]\nname="gamepkg-replay-proof"\nversion="0.0.0"\nedition="2021"\n[lib]\npath="lib.rs"\n[dependencies]\nanyhow="='+anyhow+'"\n')
    environment=dict(os.environ,CARGO_TARGET_DIR=str(project/'target/replay-proof'),RUSTFLAGS='-Dwarnings',GAMEPKG_ZELDA_REPLAY_ORACLE=str(oracle),ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    command=['cargo','test','--offline','--release','--manifest-path',str(root/'Cargo.toml'),'--lib','--no-run','--message-format=json']
    if args.bits32:
        command+=['--target','i686-unknown-linux-gnu']
        environment['CARGO_TARGET_I686_UNKNOWN_LINUX_GNU_LINKER']=args.linker
    build=subprocess.run(command,env=environment,capture_output=True,text=True,timeout=180)
    if build.returncode:raise RuntimeError(build.stderr+build.stdout[-4000:])
    artifacts=[json.loads(line) for line in build.stdout.splitlines() if line.startswith('{')]
    binaries=[Path(item['executable']) for item in artifacts if item.get('executable') and item.get('reason')=='compiler-artifact']
    assert len(binaries)==1,binaries
    for binary in [oracle,binaries[0]]:
        elf=binary.read_bytes()[:6]
        assert elf[:4]==b'\x7fELF' and elf[4]==(1 if args.bits32 else 2)
    def run(*flags,expected):
        result=subprocess.run([str(binaries[0]),*flags],env=environment,capture_output=True,text=True,timeout=180)
        if result.returncode:raise RuntimeError(result.stdout+result.stderr)
        assert f'test result: ok. {expected} passed; 0 failed;' in result.stdout,result.stdout
        print(result.stdout,end='')
    run(expected=9)
    run('--ignored','--exact','tests::original_reader_matches_all_frame_inputs_ram_hashes_and_final_ram_bytes',expected=1)
    run('--ignored','--exact','tests::original_recorder_matches_streamed_wire_and_metadata',expected=1)
    if not args.bits32:
        red=subprocess.run([str(oracle),'--truncated-patch'],env=environment,capture_output=True,text=True,timeout=30)
        assert red.returncode!=0 and 'LEGACY_TRUNCATED_REPLAY_PATCH' in red.stdout and 'heap-buffer-overflow' in red.stderr,red.stdout+red.stderr
        print('Original truncated-patch reader reproduces its ASan heap-buffer-overflow; bounded Rust framing rejects that input.')
    print('Original replay matches 12802 frame traces and 27 complete final RAM images at pointer width',32 if args.bits32 else 64)
