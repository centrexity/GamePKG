"""Pinned original Roboto/SDL_ttf speed-label raster reference; maintainer only."""
from pathlib import Path
import argparse,hashlib,json,struct,subprocess,tempfile,zlib
parser=argparse.ArgumentParser();parser.add_argument('original',type=Path);parser.add_argument('--write-source',type=Path);args=parser.parse_args()
font=args.original/'assets/font/static/Roboto-Medium.ttf';source=args.original/'src/rendering.rs'
assert hashlib.sha256(font.read_bytes()).hexdigest()=='616eee3f9b60ec0196e09700ebca48a375da3323c4d5872e2b982170969ac90e'
assert hashlib.sha256(source.read_bytes()).hexdigest()=='51b6c75d19ed56c67412daee59f5cae90ace2586902c753c2598935f2702fb83'
text=source.read_text();fragment=text[text.index('    // Draw speed text'):text.index('\n}\n',text.index('    // Draw speed text'))]
c=r'''
#include <SDL.h>
#include <SDL_ttf.h>
#include <stdio.h>
#include <stdlib.h>
int main(int argc,char**argv){if(argc!=4||TTF_Init())return 1;TTF_Font*f=TTF_OpenFont(argv[1],26);if(!f)return 2;
int m[7]={0};SDL_Color color={200,200,200,255};SDL_Surface*s;
if(argv[2][0]=='g'){unsigned int ch=(unsigned char)argv[3][0];if(TTF_GlyphMetrics32(f,ch,m+2,m+3,m+4,m+5,m+6))return 3;s=TTF_RenderGlyph32_Blended(f,ch,color);}
else{s=TTF_RenderUTF8_Blended(f,argv[3],color);}
if(!s)return 4;SDL_Surface*p=SDL_ConvertSurfaceFormat(s,SDL_PIXELFORMAT_RGBA32,0);if(!p)return 5;m[0]=p->w;m[1]=p->h;
if(fwrite(m,sizeof(int),7,stdout)!=7)return 6;for(int y=0;y<p->h;y++)if(fwrite((unsigned char*)p->pixels+y*p->pitch,4,p->w,stdout)!=(size_t)p->w)return 7;
SDL_FreeSurface(p);SDL_FreeSurface(s);TTF_CloseFont(f);TTF_Quit();return 0;}
'''
with tempfile.TemporaryDirectory(prefix='gamepkg-speed-font-') as temp:
 root=Path(temp);(root/'capture.c').write_text(c)
 subprocess.run(['cc',root/'capture.c','-o',root/'capture',*subprocess.check_output(['pkg-config','--cflags','--libs','SDL2_ttf'],text=True).split()],check=True)
 def capture(kind,label):
  raw=subprocess.check_output([root/'capture',font,kind,label]);width,height,*metrics=struct.unpack('<7i',raw[:28]);pixels=raw[28:];assert len(pixels)==width*height*4
  return dict(width=width,height=height,metrics=metrics,rgba_zlib_hex=zlib.compress(pixels).hex(),rgba_sha256=hashlib.sha256(pixels).hexdigest())
 glyphs={ch:capture('g',ch) for ch in '0123456789 px/s'}
 cases=[dict(text=str(speed)+' px/s',**capture('t',str(speed)+' px/s')) for speed in [0,1,9,10,59,60,99,100,300,600,799,800,1200,1399,1400,1800,9999,2147483647]]
 # Verify that these non-overlapping glyphs reproduce SDL_ttf's complete
 # labels byte for byte, including transparent RGB and the suffix spacing.
 for case in cases:
  pixels=bytearray([200,200,200,0])*(case['width']*case['height']);x=0
  for ch in case['text']:
   glyph=glyphs[ch];raw=zlib.decompress(bytes.fromhex(glyph['rgba_zlib_hex']))
   for y in range(glyph['height']):pixels[(y*case['width']+x)*4:(y*case['width']+x+glyph['width'])*4]=raw[y*glyph['width']*4:(y+1)*glyph['width']*4]
   x+=glyph['metrics'][4]
  assert bytes(pixels)==zlib.decompress(bytes.fromhex(case['rgba_zlib_hex']))
 if args.write_source:
  rows=[];functions=[]
  for ch,glyph in glyphs.items():
   alpha=zlib.decompress(bytes.fromhex(glyph['rgba_zlib_hex']))[3::4];runs=[]
   for value in alpha:
    if runs and runs[-1][1]==value and runs[-1][0]<255:runs[-1][0]+=1
    else:runs.append([1,value])
   rows.append('        '+str(ord(ch))+' => glyph_'+str(ord(ch))+'(),')
   functions.append('fn glyph_'+str(ord(ch))+"()-> &'static [u8] {&["+','.join(str(v) for pair in runs for v in pair)+']}\n')
  header='// Generated from pinned Roboto Medium, size 26, SDL_ttf blended glyphs.\n// Copyright 2011 The Roboto Project Authors; SIL OFL 1.1, see assets/LICENSE-ROBOTO.txt.\n'
  args.write_source.write_text(header+''.join(functions)+'fn glyph(character:u8)-> &\'static [u8] {match character {\n'+'\n'.join(rows)+'\n        _ => &[],\n}}\n'+r"""
pub struct Label {pub width:u32,pub pixels:Vec<u8>}
pub fn raster(speed:u32)->Label {
    let mut digits=[48u8;10];let mut count=1;let mut value=speed;
    digits[9]=48+(value%10) as u8;value/=10;
    while value>0 && count<10 {digits[9-count]=48+(value%10) as u8;value/=10;count+=1;}
    let width=count*15+57;let mut pixels=Vec::new();pixels.resize(width*31*4,0u8);
    for pixel in pixels.chunks_mut(4) {pixel[0]=200;pixel[1]=200;pixel[2]=200;}
    let mut label=Vec::new();label.extend_from_slice(&digits[10-count..]);label.extend_from_slice(b" px/s");
    let mut left=0;
    for character in label {
        let w=match character {32=>6,120=>13,47=>10,115=>13,_=>15};let mut index=0;
        for pair in glyph(character).chunks(2) {
            for _ in 0..pair[0] {let x=index%w;let y=index/w;pixels[(y*width+left+x)*4+3]=pair[1];index+=1;}
        }
        left+=w;
    }
    Label{width:width as u32,pixels}
}
pub fn palette_number(index:u8)->Label {
    let mut pixels=Vec::new();pixels.resize(15*31*4,0u8);
    if index<10 {
        let mut offset=0;
        for pair in glyph(48+index).chunks(2) {
            for _ in 0..pair[0] {pixels[offset*4+3]=pair[1];offset+=1;}
        }
    }
    Label{width:15,pixels}
}
""")
 if args.write_source:subprocess.run(['rustfmt','--edition','2021',args.write_source],check=True)
 print(json.dumps(dict(revision='9d613e78c9ed5657ae7fda9206cd4a82a893b7c8',font_sha256=hashlib.sha256(font.read_bytes()).hexdigest(),fragment_sha256=hashlib.sha256(fragment.encode()).hexdigest(),font_size=26,color=[200,200,200,255],display_scale=0.6,sdl_ttf_version=subprocess.check_output(['pkg-config','--modversion','SDL2_ttf'],text=True).strip(),glyphs=glyphs,cases=cases),indent=2))
