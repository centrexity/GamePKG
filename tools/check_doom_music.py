"""Maintainer music qualification; optional WAD is read in place, never extracted."""
from pathlib import Path
import argparse,os,subprocess,tempfile
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--wad',type=Path)
parser.add_argument('--complete',action='store_true',help='render every WAD song through end-of-track and cross the first loop boundary')
parser.add_argument('--32-bit',dest='bits32',action='store_true')
parser.add_argument('--cc',default='cc')
parser.add_argument('--runtime-libraries',type=Path)
args=parser.parse_args()
if args.complete and not args.wad:parser.error('--complete requires --wad')
project=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-doom-music-') as temporary:
    binary=Path(temporary)/'music'
    flags=['-m32','-DGAMEPKG_TEST_32','-fsanitize=undefined','-fsanitize-undefined-trap-on-error'] if args.bits32 else ['-fsanitize=address,undefined']
    source=project/'tests'/('doom_music_wad.c' if args.wad else 'doom_music.c')
    command=[args.cc,'-O2','-ffunction-sections','-Wl,--gc-sections',*flags,'-DFEATURE_SOUND','-DGAMEPKG_OPL_TEST']
    command += ['-I'+str(project/path) for path in ['engines/doom/upstream','engines/doom/music','engines/doom/music/upstream']]
    command += [str(source)]
    command += [str(project/'engines/doom/music'/path) for path in ['music_opl.c','midi_memory.c','music_mus.c','opl_memory.c','opl_synth.c']]
    command += [str(project/'engines/doom/upstream/memio.c'),'-o',str(binary)]
    subprocess.run(command,check=True)
    if args.bits32:assert binary.read_bytes()[:5]==b'\x7fELF\x01','actual ELF32 required'
    environment=dict(os.environ,ASAN_OPTIONS='detect_leaks=1',UBSAN_OPTIONS='halt_on_error=1')
    if args.runtime_libraries:environment['LD_LIBRARY_PATH']=str(args.runtime_libraries.resolve())
    command=[str(binary)]
    if args.wad:command.append(str(args.wad.resolve()))
    if args.complete:command.append('--complete')
    if args.wad:
        subprocess.run(command,env=environment,check=True)
    else:
        # Parser mutations deliberately emit diagnostics; show them only on failure.
        log=Path(temporary)/'qualification.log'
        with log.open('wb') as output:
            result=subprocess.run(command,env=environment,stdout=output,stderr=subprocess.STDOUT)
        if result.returncode:
            with log.open('rb') as output:
                output.seek(max(0,log.stat().st_size-8192))
                print(output.read().decode('utf-8',errors='replace'))
            result.check_returncode()
print('Music qualification passed ('+('actual 32-bit' if args.bits32 else 'host-width sanitizers')+').')
