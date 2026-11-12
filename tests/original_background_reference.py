"""Independent original SDL photos at the actual viewport, with frozen controls.

The six original RGBA hashes, not current player output, identify the source
photographs. Old 1280x720 goldens still control reconstruction and modulation.
Grid lines use original SDL operations and scaling, including both endpoints.
"""
from pathlib import Path
import hashlib,json,math,os,subprocess
from PIL import Image,ImageDraw

# Decoded independently from the six checksum-pinned upstream PNGs. Do not
# derive the expected full-resolution pixels solely from mutable tile metadata.
ORIGINAL_RGBA_SHA256=(
 '9ce3908ca652b1f5e8dad1456269a9c38249498d6bc63c438c3dae96ae8e42af',
 '34739205a3fd404f3373bcf057c3b81ce68d00dfdd045b2083f7fd3fe2ee6fa8',
 '802609d84afa682f814c80c42d228d4c19ee9ceb0e1253110c77e48a85bfcd9a',
 'ceb5f32440ff64d24530121b84cb5f3c13bfa7d28e5f3c104bc4e4c2b5aa1da3',
 '1ef0f5d22c2471fa57ce725cb966992dda60226209dd7f167b0c5fb7f4b0a237',
 '0abd8a6f51d34ef1bf3cce0f4c5253490a221653072a97f27e7892dbb0202b26',
)

class OriginalBackgroundReference:
 def __init__(self,project,folder,env,editor=False,grid=False):
  self.project=Path(project);self.folder=Path(folder);self.folder.mkdir();self.env=dict(env,SDL_VIDEODRIVER='x11',SDL_AUDIODRIVER='dummy',SDL_RENDER_DRIVER='opengl');self.editor=editor;self.grid=grid;self.cache={};self.photos=[];self.controls=[]
  self.env.pop('SDL_RENDER_SCALE_QUALITY',None)
  metadata=json.loads((self.project/'ports/arkanoo/assets/ASSET_PROVENANCE.json').read_text())['original_backgrounds'];frozen=json.loads((self.project/'tests/fixtures/arkanoo-image-backgrounds.json').read_text())
  assert frozen['revision']=='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8' and frozen['original_size']==[2816,1536]
  self.binary=self.folder/'sdl-background-reference'
  subprocess.run(['cc','-Wall','-Wextra','-Werror',str(self.project/'tests/reference/sdl_background_scene.c'),'-o',str(self.binary),*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_image'],text=True).split()],check=True)
  self.grid_mask=Image.new('L',(1280,720));drawing=ImageDraw.Draw(self.grid_mask)
  for row in range(11):drawing.line([(40,80+row*20),(1240,80+row*20)],fill=255,width=1)
  for col in range(21):drawing.line([(40+col*60,80),(40+col*60,280)],fill=255,width=1)
  for case in frozen['cases']:
   level=case['level'];source=next(s for s in metadata['sources'] if s['source']==f'background{level}.png');assert source['source_sha256']==case['original_sha256'] and source['size']==[2816,1536] and source['rgba_sha256']==ORIGINAL_RGBA_SHA256[level-1]
   photo=Image.new('RGBA',(2816,1536));tiles=[t for t in metadata['tiles'] if t['asset'].startswith(f'background{level}-')];assert len(tiles)==24
   for tile in tiles:
    path=self.project/'ports/arkanoo/assets'/tile['asset'];assert hashlib.sha256(path.read_bytes()).hexdigest()==tile['encoded_sha256']
    with Image.open(path) as image:image=image.convert('RGBA')
    assert list(image.size)==tile['size'] and hashlib.sha256(image.tobytes()).hexdigest()==tile['rgba_sha256']
    x,y,w,h=tile['core'];left,top,_,_=tile['source_rectangle'];photo.paste(image.crop((x-left,y-top,x-left+w,y-top+h)),(x,y))
   assert hashlib.sha256(photo.tobytes()).hexdigest()==source['rgba_sha256'];path=self.folder/f'original-background{level}.png';photo.save(path);self.photos.append(path)
   for label,alpha,overlay in [('bright',255,0),('game',64,0),('editor',255,153)]:
    raw=self.render(level-1,1280,720,(0,0,1280,720),alpha,overlay,False);assert hashlib.sha256(raw).hexdigest()==case[label+'_rgba_sha256'],(level,label)
    if label=='editor':
     image=Image.frombytes('RGBA',(1280,720),raw);image.paste((80,80,100,255),(0,0,1280,720),self.grid_mask);assert hashlib.sha256(image.tobytes()).hexdigest()==case['editor_grid_rgba_sha256'],(level,'grid')
     actual_grid=self.render(level-1,1280,720,(0,0,1280,720),255,153,False,True);assert hashlib.sha256(actual_grid).hexdigest()==case['editor_grid_rgba_sha256'],(level,'SDL grid')
   self.controls.append(dict(level=level,original_rgba_sha256=source['rgba_sha256'],frozen_bright_game_editor_grid='exact'))
 def render(self,case,width,height,destination,alpha,overlay,clear=True,grid=False):
  raw=subprocess.check_output([str(self.binary),str(self.photos[case]),str(width),str(height),*map(str,destination),str(alpha),str(overlay),str(int(clear)),str(int(grid))],env=self.env,timeout=30);assert len(raw)==width*height*4;return raw
 def pixel(self,case,width,height,x,y):
  key=(case,width,height)
  if key not in self.cache:
   scale=min(width/640,height/480);ox=(width-640*scale)/2;oy=(height-480*scale)/2
   # Raster viewports use integral physical edges. Preserve fractional scene
   # coordinates inside those edges rather than resampling an old screenshot.
   left=math.floor(ox+.5);top=math.floor(oy+.5);right=math.floor(ox+640*scale+.5);bottom=math.floor(oy+480*scale+.5)
   self.cache[key]=self.render(case,width,height,(left,top+60*(bottom-top)/480,right-left,360*(bottom-top)/480),255 if self.editor else 64,153 if self.editor else 0,grid=self.grid)
  raw=self.cache[key];at=(y*width+x)*4;return list(raw[at:at+3])
 def sample(self,case,width,height,x,y,sx,sy):
  return self.pixel(case,width,height,x,y)
