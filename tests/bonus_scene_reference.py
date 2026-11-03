"""Original primitive pixels over original full-resolution SDL backgrounds.

Only the background's destination changes with the viewport. Sprite samples
retain the original source-pixel/nearest-boundary contract. No player output or
production icon texture supplies expected sprite colors.
"""
from pathlib import Path
import hashlib,json,math,os,subprocess,zlib
from PIL import Image

class BonusSceneReference:
 def __init__(self,project,folder,env):
  self.project=Path(project);self.folder=Path(folder);self.folder.mkdir();self.env=dict(env,SDL_VIDEODRIVER='x11',SDL_AUDIODRIVER='dummy',SDL_RENDER_DRIVER='opengl');self.backgrounds={};self.expected_cache={}
  metadata=json.loads((self.project/'ports/arkanoo/assets/ASSET_PROVENANCE.json').read_text())['original_backgrounds'];source=next(value for value in metadata['sources'] if value['source']=='background1.png')
  assert source['source_sha256']=='21c34d3db19f1d2f8560bada107af03762a4003dfbc2c9a3c9b80b0bc9b3f8b2' and source['size']==[2816,1536] and source['rgba_sha256']=='9ce3908ca652b1f5e8dad1456269a9c38249498d6bc63c438c3dae96ae8e42af'
  photo=Image.new('RGBA',(2816,1536));tiles=[t for t in metadata['tiles'] if t['asset'].startswith('background1-')];assert len(tiles)==24
  for tile in tiles:
   path=self.project/'ports/arkanoo/assets'/tile['asset'];assert hashlib.sha256(path.read_bytes()).hexdigest()==tile['encoded_sha256']
   with Image.open(path) as image:image=image.convert('RGBA')
   assert list(image.size)==tile['size'] and hashlib.sha256(image.tobytes()).hexdigest()==tile['rgba_sha256']
   x,y,w,h=tile['core'];left,top,_,_=tile['source_rectangle'];photo.paste(image.crop((x-left,y-top,x-left+w,y-top+h)),(x,y))
  assert hashlib.sha256(photo.tobytes()).hexdigest()==source['rgba_sha256'];self.photo=self.folder/'original-background.png';photo.save(self.photo)
  frozen=json.loads((self.project/'tests/fixtures/arkanoo-bonus.json').read_text());trace=json.loads((self.project/'tests/fixtures/arkanoo-bonus-commands.json').read_text());assert trace['revision']==frozen['revision'] and trace['fragment_sha256']==frozen['fragment_sha256'] and trace['rendering_sha256']==frozen['rendering_sha256'];parts=[]
  assert sorted(case['kind'] for case in trace['cases'])==list(range(4))
  for case in trace['cases']:
   raw=zlib.decompress(bytes.fromhex(case['commands_zlib_hex']));original=next(c for c in frozen['cases'] if c['kind']==case['kind'] and c['position']==0)
   assert len(raw)<1048576 and hashlib.sha256(raw).hexdigest()==case['commands_sha256']==original['commands_sha256'] and len(raw.splitlines())==case['primitive_count']==original['primitive_count'];parts.append(b'K '+str(case['kind']).encode()+b'\n'+raw)
  self.trace=self.folder/'original-commands.txt';self.trace.write_bytes(b''.join(parts));self.binary=self.folder/'sdl-reference'
  subprocess.run(['cc','-Wall','-Wextra','-Werror',str(self.project/'tests/reference/sdl_bonus_scene.c'),'-o',str(self.binary),*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image'],text=True).split()],check=True)
 def background(self,width,height):
  key=(width,height)
  if key not in self.backgrounds:
   scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2
   raw=subprocess.check_output([str(self.binary),'background',str(self.photo),str(width),str(height),str(ox),str(oy+60*scale),str(640*scale),str(360*scale)],env=self.env,timeout=30);assert len(raw)==width*height*4;self.backgrounds[key]=raw
  return self.backgrounds[key]
 def background_pixel(self,width,height,x,y):
  assert 0<=x<width and 0<=y<height;raw=self.background(width,height);at=(y*width+x)*4;return list(raw[at:at+3])
 @staticmethod
 def texels(coordinate,extent):
  choices=[math.floor(coordinate)]
  if abs(coordinate-round(coordinate))<.00001:choices.append(round(coordinate)-1)
  return sorted({max(0,min(extent-1,c)) for c in choices})
 def bonus_samples(self,shift,width,height,relative):
  key=(shift,width,height,tuple(relative))
  if key in self.expected_cache:return self.expected_cache[key]
  scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2;requests=[];lengths=[]
  for kind in range(4):
   for dx,dy in relative:
    x=200+kind*50+shift+dx;y=220+dy;px=round(ox+x*scale);py=round(oy+y*scale);left=ox+(200+kind*50+shift)*scale;top=oy+220*scale;rgb=self.background_pixel(width,height,px,py);count=0
    for sx in self.texels((px+.5-left)*2/scale,41):
     for sy in self.texels((py+.5-top)*2/scale,41):requests.append(' '.join(map(str,[kind,sx,sy,*rgb])));count+=1
    lengths.append(count)
  data=subprocess.check_output([str(self.binary),'samples',str(self.trace)],input=('\n'.join(requests)+'\n').encode(),env=self.env,timeout=30);assert len(data)==len(requests)*4;result=[];offset=0
  for length in lengths:result.append([list(data[(offset+i)*4:(offset+i)*4+3]) for i in range(length)]);offset+=length
  self.expected_cache[key]=result;return result
