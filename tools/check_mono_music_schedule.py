"""Trace original SDL_mixer mono put sizes using generated private test audio."""
from pathlib import Path
import os
import subprocess
import tempfile

PROJECT = Path(__file__).resolve().parents[1]
flags = subprocess.check_output(['pkg-config', '--cflags', '--libs', 'SDL2_mixer', 'sdl2'], text=True).split()
with tempfile.TemporaryDirectory(prefix='gamepkg-mono-schedule-') as temporary:
    root = Path(temporary)
    source = root/'mono.mp3'
    subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i',
                    'sine=frequency=997:sample_rate=22050:duration=0.25', '-ac', '1',
                    '-codec:a', 'libmp3lame', '-b:a', '32k', str(source)], check=True)
    executable, trace = root/'music', root/'trace.so'
    subprocess.run(['cc', '-std=c11', '-Wall', '-Wextra', '-Werror',
                    str(PROJECT/'tools/audio-decoder/music_reference.c'), '-o', str(executable), *flags], check=True)
    subprocess.run(['cc', '-std=c11', '-Wall', '-Wextra', '-Werror', '-shared', '-fPIC',
                    str(PROJECT/'tools/audio-decoder/stream_trace.c'), '-o', str(trace), *flags, '-ldl'], check=True)
    for rate in [44100, 48000]:
        result = subprocess.run([str(executable), str(source), str(rate), str(root/'disk.raw')],
                                env=dict(os.environ, LD_PRELOAD=str(trace)), stdout=subprocess.DEVNULL,
                                stderr=subprocess.PIPE, text=True, timeout=35, check=True)
        formats = [list(map(int, line.split()[1:])) for line in result.stderr.splitlines() if line.startswith('TRACE_FORMAT ')]
        puts = [int(line.split()[1]) for line in result.stderr.splitlines() if line.startswith('TRACE_PUT ')]
        assert formats == [[32784, 1, 22050, 32784, 2, rate]], formats
        assert len(puts) == 3 and puts[:2] == [4096, 4096] and 0 < puts[-1] <= 4096 and puts[-1] % 2 == 0, puts
        print(f'Original mono Music API at {rate} Hz: 2048-frame full puts and bounded final put verified.')
