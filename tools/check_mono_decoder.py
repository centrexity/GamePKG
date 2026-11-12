"""Generated mono MP3 qualification against actual SDL conversion, all input rates.

Maintainer-only; fixtures and PCM remain private and are removed on completion.
Build the decoder companion with its existing 6 MiB maximum first.
"""
from pathlib import Path
import argparse
import json
import os
import subprocess
import tempfile
import sys

PROJECT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--decoder',type=Path,default=PROJECT/'tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm')
parser.add_argument('--worker',action='store_true',help='also verify signed-package output-clock controls and both worker output rates')
parser.add_argument('--null-device',action='store_true',help='also exercise real CPAL callbacks using a private ALSA null device (requires --worker)')
args=parser.parse_args()
if args.null_device and (not args.worker or sys.platform!='linux'):parser.error('--null-device requires --worker on Linux')
env=dict(os.environ,GAMEPKG_AUDIO_DECODER_WASM=str(args.decoder.resolve()),
         GAMEPKG_BUNDLED_AUDIO_DECODER=str(args.decoder.resolve()),
         GAMEPKG_BUNDLED_AUDIO_CONVERTER=os.environ.get('GAMEPKG_BUNDLED_AUDIO_CONVERTER',str(PROJECT/'tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm')))
compiled=subprocess.run(['cargo','test','--release','--all-features','--test','audio_decoder','--no-run','--message-format=json'],cwd=PROJECT,env=env,capture_output=True,text=True,timeout=300)
assert compiled.returncode==0,compiled.stderr
artifacts=[json.loads(line) for line in compiled.stdout.splitlines() if line.startswith('{')]
executable=next(item['executable'] for item in artifacts if item.get('reason')=='compiler-artifact' and item.get('target',{}).get('name')=='audio_decoder' and item.get('executable'))
worker=None
if args.worker:
    result=subprocess.run(['cargo','test','--release','--all-features','--test','music_worker','--no-run','--message-format=json'],cwd=PROJECT,env=env,capture_output=True,text=True,timeout=300)
    assert result.returncode==0,result.stderr
    artifacts=[json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
    worker=next(item['executable'] for item in artifacts if item.get('reason')=='compiler-artifact' and item.get('target',{}).get('name')=='music_worker' and item.get('executable'))
with tempfile.TemporaryDirectory(prefix='gamepkg-mono-decode-') as temporary:
    root=Path(temporary)
    null=root/'null-alsa.conf'
    if args.null_device:null.write_text('pcm.!default { type null }\n')
    oracle=root/'sdl'
    flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()
    subprocess.run(['cc','-std=c11','-Wall','-Wextra','-Werror',str(PROJECT/'tools/audio-decoder/resample_reference.c'),'-o',str(oracle),*flags],check=True)
    for rate in [8000,11025,12000,16000,22050,24000,32000,44100,48000]:
        source=root/f'mono-{rate}.mp3'
        subprocess.run(['ffmpeg','-v','error','-f','lavfi','-i',f'sine=frequency=997:sample_rate={rate}:duration=0.25','-ac','1','-codec:a','libmp3lame','-q:a','4',str(source)],check=True)
        metadata=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_entries','stream=sample_rate,channels','-of','json',str(source)]))['streams'][0]
        assert int(metadata['sample_rate'])==rate and metadata['channels']==1,metadata
        selected=dict(env,GAMEPKG_AUDIO_MONO_INPUT=str(source),GAMEPKG_AUDIO_MONO_RATE=str(rate),GAMEPKG_SDL_RESAMPLE_REFERENCE=str(oracle))
        result=subprocess.run([executable,'mono_original_mixer_matches_sdl_conversion_and_rewinds_with_equal_channels','--exact','--ignored','--nocapture'],cwd=PROJECT,env=selected,capture_output=True,text=True,timeout=30)
        assert result.returncode==0,result.stdout+'\n'+result.stderr
        print(f'Mono {rate} Hz: both output rates match SDL PCM exactly; varied reads, rewind and 6 MiB bound pass.',flush=True)

        if worker:
            result=subprocess.run([worker,'mono_signed_package_worker_controls_and_full_output_match_sdl','--exact','--ignored','--nocapture'],cwd=PROJECT,env=selected,capture_output=True,text=True,timeout=30)
            assert result.returncode==0,result.stdout+'\n'+result.stderr
            print(f'Mono {rate} Hz worker: signed asset loading, exact SDL output, controls, replay and reservation cleanup pass.',flush=True)

        if args.null_device:
            device=root/f'mono-device-{rate}.mp3'
            subprocess.run(['ffmpeg','-v','error','-f','lavfi','-i',f'sine=frequency=997:sample_rate={rate}:duration=3','-ac','1','-codec:a','libmp3lame','-q:a','4',str(device)],check=True)
            selected=dict(env,GAMEPKG_AUDIO_MONO_INPUT=str(device),GAMEPKG_AUDIO_MONO_RATE=str(rate),ALSA_CONFIG_PATH=str(null))
            result=subprocess.run([worker,'mono_actual_device_callback_controls_both_worker_rates_without_game_ticks','--exact','--ignored','--nocapture'],cwd=PROJECT,env=selected,capture_output=True,text=True,timeout=30)
            assert result.returncode==0,result.stdout+'\n'+result.stderr
            print(f'Mono {rate} Hz device: both output rates, master mute, midstream pause/resume, stop and reservation cleanup pass with zero game ticks.',flush=True)
