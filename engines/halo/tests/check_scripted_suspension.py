"""Asset-free maintainer tests of actual reviewed protocol functions; no player Python."""
import argparse,hashlib,json,os,re,subprocess
from pathlib import Path

HERE=Path(__file__).resolve().parent
def block(text,anchor,semicolon=False):
    start=text.index(anchor)
    # Keep offsets while excluding comments/strings from brace matching.
    lexical=re.sub(r'/\*.*?\*/|//[^\n]*|"(?:\\.|[^"\\])*"',lambda m:' '*len(m[0]),text,flags=re.S)
    first=lexical.index('{',start);depth=1;end=first+1
    while depth:
        depth+=(lexical[end]=='{')-(lexical[end]=='}');end+=1
    if semicolon:
        assert text[end]==';';end+=1
    return text[start:end]+'\n'

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--profile',choices=['native32','sanitized64'],required=True);p.add_argument('--cc',default='clang-18')
    a=p.parse_args();source=a.source.resolve();out=a.output.resolve();out.mkdir(parents=True,exist_ok=True)
    paths={key:source/path for key,path in {'network':'port/linux/game/network_distributed.c','header':'port/linux/game/network_distributed.h',
        'players':'source/game/players.h','units':'source/units/units.h','setter':'source/units/units.c','version':'port/linux/include/halo_port_limits.h'}.items()}
    texts={key:path.read_text() for key,path in paths.items()}
    assert re.search(r'#define HALO_PORT_NETWORK_VERSION 34\b',texts['version'])
    pieces=[block(texts['players'],'enum\n{\n\t_player_powerup_active_camouflage',True),
        block(texts['units'],'enum\n{\n\t_unit_actively_controlled_bit',True),
        block(texts['header'],'struct distributed_vector\n',True)]
    for anchor in ['enum\n{\n\t/* the player has a unit that is alive */','enum\n{\n\t/* (alive) camouflaged, and doubly so */',
        'enum\n{\n\t/* (alive) the vehicle it rides and the seat */','struct distributed_unit_state\n']:
        pieces.append(block(texts['network'],anchor,True))
    size=block(texts['network'],"enum\n{\n\t/* a unit state's bytes on the wire",True)
    pieces.append(size.split('\tDISTRIBUTED_RELAYED_ACTION_MINIMUM_SIZE',1)[0]+'};\n')
    pieces.append(block(texts['setter'],'void unit_scripting_suspended(\n'))
    for anchor in ['static byte *distributed_put(\n','static boolean distributed_take(\n','static word distributed_unit_state_write(\n',
        'static word distributed_unit_state_read(\n','static void distributed_apply_scripted_suspension(\n']:
        pieces.append(block(texts['network'],anchor))
    fragments=out/'reviewed_snapshot.inc';fragments.write_text('\n'.join(pieces))
    flags=['--target=i686-linux-gnu','-m32','-isystem','/usr/include/x86_64-linux-gnu'] if a.profile=='native32' else ['-fsanitize=address,undefined','-fno-omit-frame-pointer']
    command=[a.cc,*flags,'-std=gnu17','-O2','-g','-ffp-contract=off','-Wall','-Wextra','-Werror','-I'+str(out),str(HERE/'scripted_suspension.c'),'-o',str(out/'check')]
    with (out/'build.log').open('w') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
    with (out/'run.log').open('w') as log:subprocess.run([str(out/'check')],stdout=log,stderr=subprocess.STDOUT,check=True,timeout=60,
        env=dict(os.environ,ASAN_OPTIONS='detect_leaks=1:halt_on_error=1',UBSAN_OPTIONS='halt_on_error=1:print_stacktrace=1'))
    record=dict(profile=a.profile,result='pass',sources={key:hashlib.sha256(path.read_bytes()).hexdigest() for key,path in paths.items()},
        fragments_sha256=hashlib.sha256(fragments.read_bytes()).hexdigest())
    (out/'result.json').write_text(json.dumps(record,indent=2)+'\n');print((out/'run.log').read_text(),end='')
if __name__=='__main__':main()
