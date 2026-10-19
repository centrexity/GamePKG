"""Maintainer checks for bounded embedding and stale optional build inputs."""
from pathlib import Path
import os
import subprocess
import tempfile

project = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='gamepkg-embed-check-') as temporary:
    root = Path(temporary)
    executable = root/('embed.exe' if os.name == 'nt' else 'embed')
    subprocess.run(['rustc', '--edition=2021', str(project/'build.rs'), '-o', str(executable)], check=True, timeout=60)
    environment = dict(os.environ, OUT_DIR=str(root))
    environment.pop('GAMEPKG_BUNDLED_AUDIO_DECODER', None)
    environment.pop('GAMEPKG_BUNDLED_AUDIO_CONVERTER', None)
    def run(path=None, good=True):
        env = dict(environment)
        if path is not None:
            env['GAMEPKG_BUNDLED_AUDIO_DECODER'] = str(path)
        result = subprocess.run([str(executable)], env=env, capture_output=True, text=True, timeout=10)
        assert (result.returncode == 0) == good, result.stdout + result.stderr
    output = root/'audio-decoder.wasm'
    run()
    assert output.read_bytes() == b''
    source = root/'companion.wasm'
    for data in [b'', b'bad wasm', b'\0asm\x02\0\0\0', b'\0asm\x01\0\0\0' + bytes(4*1024*1024)]:
        source.write_bytes(data)
        run(source, good=False)
    run(root/'missing.wasm', good=False)
    # Build-time syntax guard; full import/memory/API validation belongs to the
    # factory and packaging check, not this header-only build script.
    for data in [b'\0asm\x01\0\0\0', b'\0asm\x01\0\0\0\0\x01\0']:
        source.write_bytes(data)
        run(source)
        assert output.read_bytes() == data
    run()
    assert output.read_bytes() == b'', 'removed embedding input retained old bytes'
    converter = root/'audio-converter.wasm'
    for data,good in [(b'bad',False),(b'\0asm\x01\0\0\0',True),(b'\0asm\x01\0\0\0'+bytes(4*1024*1024),False)]:
        source.write_bytes(data);result=subprocess.run([str(executable)],env=dict(environment,GAMEPKG_BUNDLED_AUDIO_CONVERTER=str(source)),capture_output=True,text=True,timeout=10)
        assert (result.returncode==0)==good,result.stderr
        if good:assert converter.read_bytes()==data
    run();assert converter.read_bytes()==b'', 'removed converter embedding retained old bytes'
print('Embedding checks passed: optional reset, changed input, missing/bad/oversized modules and bounded core header.')
