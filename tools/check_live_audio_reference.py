"""Maintainer independent SDL reference for continuous output after track completion."""
from pathlib import Path
import argparse,hashlib,json,subprocess,tempfile
parser=argparse.ArgumentParser();parser.add_argument('--write-fixture',action='store_true');args=parser.parse_args()
project=Path(__file__).resolve().parents[1];source=project/'tools/audio-decoder/live_mix_reference.c';fixture=project/'tests/fixtures/sdl-live-audio-reference.json'
flags=subprocess.check_output(['pkg-config','--cflags','--libs','sdl2'],text=True).split()
with tempfile.TemporaryDirectory(prefix='gamepkg-live-reference-') as temporary:
    executable=Path(temporary)/'reference'
    subprocess.run(['cc','-O2','-Wall','-Wextra','-Werror',str(source),'-o',str(executable),*flags],check=True)
    output=subprocess.check_output([str(executable)],timeout=10)
records=[]
for music_volume,effect_volume in [(128,v) for v in [0,1,63,64,127,128]]+[(0,64),(64,64)]:
    pcm,output=output[:8192*4],output[8192*4:];assert len(pcm)==8192*4
    records.append(dict(music_volume=music_volume,effect_volume=effect_volume,frames=8192,sha256=hashlib.sha256(pcm).hexdigest()))
assert not output
record=dict(sdl_version=subprocess.check_output(['pkg-config','--modversion','sdl2'],text=True).strip(),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),music_frames=6000,source_frames=8000,output_prefix_frames=8192,cases=records)
if args.write_fixture:fixture.write_text(json.dumps(record,indent=2)+'\n')
else:assert json.loads(fixture.read_text())==record,'Continuous SDL reference changed'
print('Independent SDL continuous output: eight complete 8192-frame prefixes include track completion, zero music, half music and four effects.')
