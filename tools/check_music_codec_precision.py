"""Private codec provenance/precision probe; never publishes or changes PCM baselines.

Without a music folder, uses one generated stereo recording. Optional original
inputs must be 44.1 or 48 kHz stereo. PCM and synthetic packages remain temporary.
Requires the built 6 MiB decoder companion and maintainer SDL/MPG123 headers.
"""
from pathlib import Path
import argparse
import array
import json
import os
import subprocess
import sys
import tempfile

PROJECT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('music',type=Path,nargs='?')
parser.add_argument('--decoder',type=Path,default=PROJECT/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm')
parser.add_argument('--float-check',action='store_true',help='also isolate MPG123 float output using explicit ties-to-even S16 conversion')
args=parser.parse_args()

def compare(left,right):
    assert left.stat().st_size==right.stat().st_size,'native decoder lengths differ'
    different=maximum=samples=0
    with left.open('rb') as a,right.open('rb') as b:
        while data:=a.read(65536):
            other=b.read(len(data));assert len(other)==len(data) and len(data)%2==0
            x=array.array('h');x.frombytes(data)
            y=array.array('h');y.frombytes(other)
            if sys.byteorder!='little':x.byteswap();y.byteswap()
            for source,target in zip(x,y):
                delta=abs(source-target);different+=delta!=0;maximum=max(maximum,delta)
            samples+=len(x)
    return dict(samples=samples,different=different,maximum_delta=maximum)

with tempfile.TemporaryDirectory(prefix='gamepkg-codec-precision-') as temporary:
    root=Path(temporary);music=args.music
    if music is None:
        music=root/'generated';music.mkdir()
        subprocess.run(['ffmpeg','-v','error','-f','lavfi','-i','aevalsrc=0.12*sin(2*PI*997*t)|0.08*sin(2*PI*613*t):s=44100:d=0.5','-codec:a','libmp3lame','-q:a','4',str(music/'stereo.mp3')],check=True)
    paths=sorted(music.glob('*.mp3'));assert paths,'No MP3 inputs'
    probe,original=root/'mpg123',root/'music'
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','libmpg123'],text=True).split()
    subprocess.run(['cc','-std=c11','-Wall','-Wextra','-Werror',str(PROJECT/'tools/audio-decoder/mpg123_reference.c'),'-o',str(probe),*flags,'-lm'],check=True)
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_mixer','sdl2'],text=True).split()
    subprocess.run(['cc','-std=c11','-Wall','-Wextra','-Werror',str(PROJECT/'tools/audio-decoder/music_reference.c'),'-o',str(original),*flags],check=True)
    choices={mode:(mode,False) for mode in ['auto','generic']}
    if args.float_check:choices.update({mode+'_float':(mode,True) for mode in ['auto','generic']})
    roots={mode:root/mode for mode in choices}
    for directory in roots.values():directory.mkdir()
    rows={}
    for path in paths:
        assert path.is_file() and not path.is_symlink()
        metadata=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_entries','stream=sample_rate,channels','-of','json',str(path)]))['streams'][0]
        assert int(metadata['sample_rate']) in [44100,48000] and metadata['channels']==2,'Use native 44.1 or 48 kHz stereo inputs'
        modes={}
        for mode in roots:
            with (roots[mode]/path.with_suffix('.s16').name).open('wb') as out:
                backend,floating=choices[mode]
                result=subprocess.run([str(probe),str(path),backend,*(['float'] if floating else [])],stdout=out,stderr=subprocess.PIPE,text=True,timeout=35,check=True)
            modes[mode]=next(line for line in result.stderr.splitlines() if line.startswith('MPG123_FORMAT '))
        actual=root/'original.s16'
        with actual.open('wb') as out:
            result=subprocess.run([str(original),str(path.resolve()),metadata['sample_rate'],str(root/'disk.raw')],stdout=out,stderr=subprocess.PIPE,text=True,timeout=35,check=True,env=dict(os.environ,SDL_MIXER_DEBUG_MUSIC_INTERFACES='1'))
        assert 'Loaded music with MPG123' in result.stderr,'Original reference selected a different backend'
        auto=roots['auto']/path.with_suffix('.s16').name
        assert actual.stat().st_size>=auto.stat().st_size
        with actual.open('rb') as src,auto.open('rb') as expected:
            while block:=expected.read(65536):assert src.read(len(block))==block,'Direct MPG123 does not match original Music API'
            while block:=src.read(65536):assert not any(block),'Non-silent callback tail'
        rows[path.name]=dict(name=path.name,formats=modes,original_music_api_exact=True,portable_vs_native=compare(auto,roots['generic']/path.with_suffix('.s16').name))
        if args.float_check:
            rows[path.name]['native_s16_vs_float']=compare(auto,roots['auto_float']/path.with_suffix('.s16').name)
            rows[path.name]['generic_s16_vs_float']=compare(roots['generic']/path.with_suffix('.s16').name,roots['generic_float']/path.with_suffix('.s16').name)
    env=dict(os.environ,GAMEPKG_AUDIO_DECODER_WASM=str(args.decoder.resolve()),GAMEPKG_BUNDLED_AUDIO_DECODER=str(args.decoder.resolve()),GAMEPKG_BUNDLED_AUDIO_CONVERTER=os.environ.get('GAMEPKG_BUNDLED_AUDIO_CONVERTER',str(PROJECT/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm')),GAMEPKG_MUSIC_INPUT_DIR=str(music.resolve()),GAMEPKG_AUDIO_MEMORY_BYTES='6291456')
    for name in ['GAMEPKG_AUDIO_OUTPUT_RATE','GAMEPKG_AUDIO_ORIGINAL_MIXER','GAMEPKG_AUDIO_PACKAGE_READERS']:env.pop(name,None)
    built=subprocess.run(['cargo','test','--release','--all-features','--test','audio_decoder','--no-run','--message-format=json'],cwd=PROJECT,env=env,capture_output=True,text=True,timeout=300)
    assert built.returncode==0,built.stderr
    artifacts=[json.loads(line) for line in built.stdout.splitlines() if line.startswith('{')]
    executable=next(item['executable'] for item in artifacts if item.get('reason')=='compiler-artifact' and item.get('target',{}).get('name')=='audio_decoder' and item.get('executable'))
    for mode in roots:
        selected=dict(env,GAMEPKG_AUDIO_REFERENCE_ROOT=str(roots[mode]))
        result=subprocess.run([executable,'original_tracks_stream_with_fixed_memory_chunk_invariance_and_rewind','--exact','--ignored','--nocapture'],cwd=PROJECT,env=selected,capture_output=True,text=True,timeout=120)
        assert result.returncode==0,result.stdout+'\n'+result.stderr
        records=[json.loads(line.split('MUSIC_JSON ',1)[1]) for line in result.stdout.splitlines() if 'MUSIC_JSON ' in line]
        assert len(records)==len(paths)
        for record in records:rows[record['name']]['nanomp3_vs_'+mode]=dict(samples=record['samples'],different=record['sdl_different'],maximum_delta=record['sdl_maximum_delta'])
    for row in rows.values():print(json.dumps(row),flush=True)
