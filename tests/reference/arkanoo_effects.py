"""Reproduce original SDL-decoded effects. Maintainer tools only; no player decoder.
Default: emit hashes/credits without modifying assets. --write-assets is explicit.
"""
from pathlib import Path
import argparse,hashlib,json,subprocess,tempfile,wave,struct
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--write-assets',type=Path);args=parser.parse_args()
items=[('ball.mp3', 'bounce.wav', 1, None, 'https://github.com/c0m4r/arkanoo/blob/9d613e78c9ed5657ae7fda9206cd4a82a893b7c8/README.md#credits', '92c1ab475858749c406d52a16da1fbc1ca613a8dab097225b1c1a56232cb102b'), ('breaking-glass.mp3', 'glass.wav', 2, None, 'https://github.com/c0m4r/arkanoo/blob/9d613e78c9ed5657ae7fda9206cd4a82a893b7c8/README.md#credits', 'b05e411ad85db49c496521796c594a88cdce10ac01ae4081909bb02b20d45067'), ('load.mp3', 'launch.wav', 3, None, 'https://github.com/c0m4r/arkanoo/blob/9d613e78c9ed5657ae7fda9206cd4a82a893b7c8/README.md#credits', '065142fbe9eec4486b87c24af5e346879ee062374bc98135e6c94112a253377f'), ('drop-sound-effect-240899.mp3', 'life-loss.wav', 5, '11325622', 'https://pixabay.com/sound-effects/film-special-effects-drop-sound-effect-240899/', 'ea82c596c2d0d14b912431f7cf886ca1017bdb76d62e173d2104ccf52be896bd'), ('swish-swoosh-woosh-sfx-27-357164.mp3', 'explosion.wav', 4, 'StereogenicStudio', 'https://pixabay.com/sound-effects/film-special-effects-swish-swoosh-woosh-sfx-27-357164/', '07ace567bee5499db9888daaf8c608302b3ce980fa7792cbb0461dd892999b30')]
source=Path(__file__).with_suffix('.c');proof=[]
with tempfile.TemporaryDirectory(prefix='gamepkg-original-effects-') as temporary:
 root=Path(temporary);binary=root/'decode';flags=subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_mixer','sdl2'],text=True).split();subprocess.run(['cc',str(source),'-o',str(binary),*flags],check=True)
 for original,name,kind,creator,url,expected in items:
  original_path=args.original/'assets'/original;digest=hashlib.sha256(original_path.read_bytes()).hexdigest();assert digest==expected,original
  pcm=subprocess.check_output([str(binary),str(original_path)]);asset=root/name
  with wave.open(str(asset),'wb') as f:f.setnchannels(2);f.setsampwidth(2);f.setframerate(48000);f.writeframes(pcm)
  if args.write_assets:
   args.write_assets.mkdir(parents=True,exist_ok=True);(args.write_assets/name).write_bytes(asset.read_bytes())
  proof.append(dict(original=original,original_sha256=digest,asset='audio/'+name,kind=kind,creator=creator,url=url,frames=len(pcm)//4,pcm_stereo_differences=sum(left!=right for left,right in struct.iter_unpack('<hh',pcm)),credit_basis='item page' if creator else 'pinned upstream README credits sounds and music to Pixabay; individual artist/item page not supplied',pcm_sha256=hashlib.sha256(pcm).hexdigest(),wav_sha256=hashlib.sha256(asset.read_bytes()).hexdigest()))
print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',upstream_credits_readme_sha256=hashlib.sha256((args.original/'README.md').read_bytes()).hexdigest(),decode_rate=44100,output_rate=48000,channels=2,format='signed 16-bit little-endian',decoder_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),sdl_version=subprocess.check_output(['pkg-config','--modversion','sdl2'],text=True).strip(),mixer_version=subprocess.check_output(['pkg-config','--modversion','SDL2_mixer'],text=True).strip(),effects=proof),indent=2))
