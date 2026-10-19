"""Original editor Roboto labels from pinned SDL_ttf; maintainer only."""
from pathlib import Path
import argparse,hashlib,json,struct,subprocess,tempfile,zlib
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--write-source',type=Path);args=parser.parse_args()
font=args.original/'assets/font/static/Roboto-Medium.ttf'
for name,digest in [('assets/font/static/Roboto-Medium.ttf','616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'),('src/rendering.rs','51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'),('src/editor.rs','ad85d563ee863a873026919d44dc9704542746e6e549ffba6d44cc70b5cd8675')]:
 assert hashlib.sha256((args.original/name).read_bytes()).hexdigest()==digest
labels=[('LEVEL EDITOR',255),('Colors:',200),('Drag Left: Draw | Drag Right: Erase | Click colors or 0-5 keys',180),*[(s,255) for s in ['Save (S)','Clear (C)','Test (T)','Load (L)','Exit (ESC)','< BG Prev','BG Next >']],*[(f'Background {i}/6',150) for i in range(1,7)],('Load Pattern',255),('Click pattern to load | ESC to cancel',180)]
c=r'''
#include <SDL.h>
#include <SDL_ttf.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char**argv){if(argc!=4||TTF_Init())return 1;TTF_Font*f=TTF_OpenFont(argv[1],26);if(!f)return 2;int shade=atoi(argv[3]);SDL_Surface*s=TTF_RenderUTF8_Blended(f,argv[2],(SDL_Color){shade,shade,shade,255});if(!s)return 3;SDL_Surface*p=SDL_ConvertSurfaceFormat(s,SDL_PIXELFORMAT_RGBA32,0);if(!p)return 4;int size[2]={p->w,p->h};if(fwrite(size,sizeof(int),2,stdout)!=2)return 5;for(int y=0;y<p->h;y++)if(fwrite((unsigned char*)p->pixels+y*p->pitch,4,p->w,stdout)!=(size_t)p->w)return 6;SDL_FreeSurface(p);SDL_FreeSurface(s);TTF_CloseFont(f);TTF_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-editor-labels-') as temp:
 root=Path(temp);(root/'capture.c').write_text(c);subprocess.run(['cc',root/'capture.c','-o',root/'capture',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf'],text=True).split()],check=True)
 cases=[];functions=[];arms=[]
 for index,(text,shade) in enumerate(labels):
  raw=subprocess.check_output([root/'capture',font,text,str(shade)]);w,h=struct.unpack('<2i',raw[:8]);pixels=raw[8:];assert len(pixels)==w*h*4 and h==31
  cases.append(dict(text=text,width=w,height=h,shade=shade,rgba_sha256=hashlib.sha256(pixels).hexdigest(),rgba_zlib_hex=zlib.compress(pixels).hex()))
  runs=[]
  for value in pixels[3::4]:
   if runs and runs[-1][1]==value and runs[-1][0]<255:runs[-1][0]+=1
   else:runs.append([1,value])
  functions.append('fn editor_label_'+str(index)+"()-> &'static [u8] {b\""+''.join('\\x'+format(v,'02x') for pair in runs for v in pair)+'\"}\n')
  arms.append(str(index)+'=>('+str(w)+','+str(shade)+',editor_label_'+str(index)+'()),')
 if args.write_source:
  args.write_source.write_text('// Generated from pinned Roboto Medium 26 / SDL_ttf.\n// Copyright 2011 The Roboto Project Authors; SIL OFL 1.1, assets/LICENSE-ROBOTO.txt.\n'+''.join(functions)+'''pub struct Label {pub width:u32,pub pixels:Vec<u8>}
pub fn editor_raster(which:u32)->Label {
 let (width,shade,runs)=match which {'''+''.join(arms)+'''_ => (0,0,&[] as &[u8])};
 let mut pixels=Vec::new();pixels.resize(width*31*4,0u8);
 for pixel in pixels.chunks_mut(4) {pixel[0]=shade;pixel[1]=shade;pixel[2]=shade;}
 let mut at=0;
 for pair in runs.chunks(2) {for _ in 0..pair[0] {pixels[at*4+3]=pair[1];at+=1;}}
 Label{width:width as u32,pixels}
}
''');subprocess.run(['rustfmt','--edition','2021',args.write_source],check=True)
 print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',font_size=26,cases=cases),indent=2))
