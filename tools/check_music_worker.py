"""Maintainer-only output-clock music worker check; no game ticks or audio device.
Creates private signed fixtures using a synthetic key; publishes no recordings.
"""
from pathlib import Path
import argparse, json, os, subprocess, tempfile
parser = argparse.ArgumentParser()
parser.add_argument('music', type=Path)
parser.add_argument('--null-device', action='store_true')
parser.add_argument('--bundled', action='store_true', help='embed the decoder at build time and remove runtime decoder paths')
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
build_environment = dict(os.environ, GAMEPKG_BUNDLED_AUDIO_DECODER=wasm) if args.bundled else None
artifacts = cargo(['test', '--release', '--test', 'music_worker', '--no-run',
                   *(['--features', 'audio'] if args.null_device else [])], build_environment)
executable = next(item['executable'] for item in artifacts if item.get('reason') == 'compiler-artifact'
                  and item.get('target', {}).get('name') == 'music_worker' and item.get('executable'))
library = None
if args.null_device:
    artifacts = cargo(['test', '--release', '--lib', '--features', 'audio', '--no-run'], build_environment)
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
    if args.bundled:
        environment['GAMEPKG_MUSIC_USE_BUNDLED'] = '1'
        environment.pop('GAMEPKG_AUDIO_DECODER_WASM', None)
        environment.pop('GAMEPKG_BUNDLED_AUDIO_DECODER', None)
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
    result = subprocess.run([executable, '--ignored', '--nocapture', '--skip', 'mono_signed_package_worker_controls_and_full_output_match_sdl', '--skip', 'mono_actual_device_callback_controls_both_worker_rates_without_game_ticks'], cwd=project, env=environment,
                            capture_output=True, text=True, timeout=120)
    assert result.returncode == 0, result.stdout + '\n' + result.stderr
    records = [json.loads(line.split('MUSIC_WORKER_JSON ', 1)[1]) for line in result.stdout.splitlines()
               if 'MUSIC_WORKER_JSON ' in line]
    assert len(records) == len(list(args.music.glob('*.mp3'))) > 0
    for record in records:
        print(json.dumps(record), flush=True)
    print('Complete signed-package playback, concurrent asset reads, original mixer/output lengths and reservation cleanup passed for', len(records), 'tracks.', flush=True)
    if args.bundled:
        print('Playback used the embedded decoder with both decoder-path environment variables removed.', flush=True)
    if args.null_device:
        assert 'MUSIC_CALLBACK_OK' in result.stdout, 'device callback verification did not run'
        assert 'ORIGINAL_MIX_CALLBACK_OK' in result.stdout, 'combined original-rate callback verification did not run'
        assert 'EFFECT_TRANSACTION_CALLBACK_OK' in result.stdout, 'signed effect transaction callback verification did not run'
        print('CPAL null-device callback: music advances with muted master gain, emits nonzero output when unmuted, and pauses/resumes without game ticks.', flush=True)
        print('Combined original-rate callback: music and effects reach the device without game ticks; stopped music does not stop effect playback; reservations release.', flush=True)
        print('Signed effect transactions: cache reuse, failed-tick denial and output-owned PCM survive cache shutdown on the actual null-device callback.', flush=True)
