"""Maintainer-only output-clock music worker check; no game ticks or audio device.
Creates private signed fixtures using a synthetic key; publishes no recordings.
"""
from pathlib import Path
import argparse, json, os, subprocess, tempfile
parser = argparse.ArgumentParser()
parser.add_argument('music', type=Path)
parser.add_argument('--null-device', action='store_true')
args = parser.parse_args()
project = Path(__file__).resolve().parents[1]
def cargo(arguments, environment=None):
    result = subprocess.run(['cargo', *arguments, '--message-format=json'], cwd=project,
                            env=environment, capture_output=True, text=True, timeout=300)
    assert result.returncode == 0, result.stderr
    return [json.loads(line) for line in result.stdout.splitlines() if line.startswith('{')]
artifacts = cargo(['build', '--locked', '--release', '--target', 'wasm32-unknown-unknown',
                   '--manifest-path', 'tools/audio-decoder/Cargo.toml'],
                  dict(os.environ, GAMEPKG_AUDIO_MAX_MEMORY_BYTES='6291456'))
wasm = next(path for item in artifacts if item.get('reason') == 'compiler-artifact'
            for path in item.get('filenames', []) if path.endswith('.wasm'))
artifacts = cargo(['test', '--release', '--test', 'music_worker', '--no-run',
                   *(['--features', 'audio'] if args.null_device else [])])
executable = next(item['executable'] for item in artifacts if item.get('reason') == 'compiler-artifact'
                  and item.get('target', {}).get('name') == 'music_worker' and item.get('executable'))
library = None
if args.null_device:
    artifacts = cargo(['test', '--release', '--lib', '--features', 'audio', '--no-run'])
    library = next(item['executable'] for item in artifacts if item.get('reason') == 'compiler-artifact'
                   and item.get('target', {}).get('name') == 'gamepkg' and item.get('executable'))
with tempfile.TemporaryDirectory(prefix='gamepkg-worker-reference-') as temporary:
    root = Path(temporary)
    flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'SDL2_mixer', 'sdl2'], text=True).split()
    subprocess.run(['cc', str(project/'tools/audio-decoder/reference.c'), '-o', str(root/'reference'), *flags], check=True)
    for path in sorted(args.music.glob('*.mp3')):
        with (root/path.with_suffix('.s16').name).open('wb') as output:
            subprocess.run([str(root/'reference'), str(path.resolve()), '44100', '48000'], stdout=output, check=True)
    environment = dict(os.environ, GAMEPKG_MUSIC_INPUT_DIR=str(args.music.resolve()),
                       GAMEPKG_AUDIO_DECODER_WASM=wasm, GAMEPKG_AUDIO_REFERENCE_ROOT=str(root))
    if args.null_device:
        null = root/'null-alsa.conf'
        null.write_text('pcm.!default { type null }\n')
        environment['ALSA_CONFIG_PATH'] = str(null)
        held_lock = subprocess.run([library, 'audio::tests::audio_output_null_device',
                                    '--ignored', '--exact', '--nocapture'], cwd=project, env=environment,
                                   capture_output=True, text=True, timeout=30)
        assert held_lock.returncode == 0, held_lock.stdout + '\n' + held_lock.stderr
        assert '1 passed' in held_lock.stdout, 'held-control callback test did not run'
        print('Actual null-device callback consumes all 1600 queued frames while the producer/control lock remains held.', flush=True)
    result = subprocess.run([executable, '--ignored', '--nocapture'], cwd=project, env=environment,
                            capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stdout + '\n' + result.stderr
    records = [json.loads(line.split('MUSIC_WORKER_JSON ', 1)[1]) for line in result.stdout.splitlines()
               if 'MUSIC_WORKER_JSON ' in line]
    assert len(records) == len(list(args.music.glob('*.mp3'))) > 0
    for record in records:
        print(json.dumps(record), flush=True)
    print('Complete signed-package playback, concurrent asset reads, original mixer/output lengths and reservation cleanup passed for', len(records), 'tracks.', flush=True)
    if args.null_device:
        assert 'MUSIC_CALLBACK_OK' in result.stdout, 'device callback verification did not run'
        assert 'ORIGINAL_MIX_CALLBACK_OK' in result.stdout, 'combined original-rate callback verification did not run'
        print('CPAL null-device callback: music advances with muted master gain, emits nonzero output when unmuted, and pauses/resumes without game ticks.', flush=True)
        print('Combined original-rate callback: music and effects reach the device without game ticks; stopped music does not stop effect playback; reservations release.', flush=True)
