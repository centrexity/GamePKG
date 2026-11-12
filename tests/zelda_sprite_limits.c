/* Exercise the pinned evaluator with unsigned plane reads and both original line caps. */
#include <assert.h>
#include <stdlib.h>
#include "engines/zelda3/upstream/snes/ppu.c"
static void prepare(Ppu *ppu, unsigned count, bool large, unsigned extra, bool unlimited, unsigned color)
{
  memset(ppu, 0, sizeof(*ppu));
  ppu->extraLeftRight = extra;
  ppu->renderFlags = kPpuRenderFlags_NewRenderer | (unlimited ? kPpuRenderFlags_NoSpriteLimits : 0);
  ppu->objSize = 0; /* Original 8/16-pixel object sizes. */
  for(unsigned i = 0; i < 128; i++) ppu->oam[i*2] = 0xf000;
  for(unsigned i = 0; i < count; i++) {
    ppu->oam[i*2] = i+1 == count ? 64 : 0;
    if(large) ppu->oam[0x100 + (i/8)] |= 2u << (2*(i%8));
  }
  for(unsigned row=0;row<8;row++) {
    ppu->vram[row] = ppu->vram[16+row] = ((color&1)?0xff:0) | ((color&2)?0xff00:0);
    ppu->vram[8+row] = ppu->vram[24+row] = ((color&4)?0xff:0) | ((color&8)?0xff00:0);
  }
  for(unsigned i=0;i<kPpuXPixels;i++) ppu->objBuffer.data[i]=0x500;
}
static uint32 rgb(unsigned color)
{
  if(!color) return 0;
  unsigned r=(color<<3)|(color>>2), g=((31-color)<<3)|((31-color)>>2);
  return (r<<16)|(g<<8)|r;
}
static void image_begin(Ppu *ppu, uint32 *output, unsigned extra, bool tall)
{
  for(unsigned i=0;i<kPpuXPixels+8;i++) output[i]=0xcccccccc;
  if(!ppu->mode) ppu->mode=1; ppu->brightness=15; ppu->lastBrightnessMult=255;
  ppu->mosaicSize=ppu->mosaicSize?ppu->mosaicSize:1; ppu->lastMosaicModulo=255;
  PpuSetExtraSideSpace(ppu,extra,extra,tall?16:0);
  PpuBeginDrawing(ppu,(uint8 *)(output+4),(256+extra*2)*4,
    ppu->renderFlags|(tall?kPpuRenderFlags_Height240:0));
}
static void guards(const uint32 *output, unsigned width)
{
  for(unsigned i=0;i<4;i++) assert(output[i]==0xcccccccc);
  for(unsigned i=4+width;i<kPpuXPixels+8;i++) assert(output[i]==0xcccccccc);
}
static void compose_sprites(Ppu *ppu, unsigned count, bool large, unsigned extra,
                            bool unlimited, unsigned color)
{
  uint32 output[kPpuXPixels+8];
  unsigned width=256+extra*2, cap=large?17:32, size=large?16:8;
  ppu->cgram[0x80+color]=color?(color|((31-color)<<5)|(color<<10)):0;
  ppu->screenEnabled[0]=16;
  for(unsigned tall=0;tall<=1;tall++) {
    ppu->forcedBlank=false;
    image_begin(ppu,output,extra,tall);
    ppu_runLine(ppu,1);
    for(unsigned i=0;i<width;i++) {
      int x=(int)i-(int)extra;
      bool first=count>1 && x>=0 && x<(int)size;
      bool last=count>0 && (unlimited||count<=cap) && x>=64 && x<64+(int)size;
      assert(output[4+i]==((first||last)?rgb(color):0));
    }
    guards(output,width);
    ppu->forcedBlank=true;
    image_begin(ppu,output,extra,tall);
    ppu_runLine(ppu,1);
    for(unsigned i=0;i<width;i++) assert(output[4+i]==0);
    guards(output,width);
  }
}
static void compose_backgrounds(Ppu *ppu)
{
  uint32 output[kPpuXPixels+8]; unsigned cases=0;
  for(unsigned extra=0;extra<=96;extra+=96)
   for(unsigned color=0;color<=15;color++)
    for(unsigned flip=0;flip<=1;flip++)
     for(unsigned mosaic=0;mosaic<=1;mosaic++)
      for(unsigned scroll=0;scroll<8;scroll++)
       for(unsigned tall=0;tall<=1;tall++) {
        prepare(ppu,0,false,extra,false,color);
        ppu->screenEnabled[0]=1;
        ppu->bgLayer[0].tileAdr=0x1000;
        ppu->bgLayer[0].hScroll=scroll;
        ppu->mosaicEnabled=mosaic; ppu->mosaicSize=mosaic?2:1;
        for(unsigned i=0;i<0x400;i++) ppu->vram[i]=flip?0x4000:0;
        for(unsigned row=0;row<8;row++) {
          ppu->vram[0x1000+row]=((color&1)?0xff:0)|((color&2)?0xff00:0);
          ppu->vram[0x1008+row]=((color&4)?0xff:0)|((color&8)?0xff00:0);
        }
        ppu->cgram[color]=color?(color|((31-color)<<5)|(color<<10)):0;
        image_begin(ppu,output,extra,tall);
        ppu_runLine(ppu,1);
        for(unsigned i=0;i<256+extra*2;i++) assert(output[4+i]==rgb(color));
        guards(output,256+extra*2); cases++;
       }
  assert(cases==2048);
}
static unsigned pattern(int sample, unsigned format, bool flip, unsigned scroll)
{
  int source=format==2 ? (flip?255-sample:sample)+(int)scroll : sample+(int)scroll;
  while(source<0) source+=256;
  unsigned tile=((unsigned)source/8)%32, pixel=(unsigned)source%8;
  if(format!=2 && flip) pixel=7-pixel;
  return format==0?(tile+pixel)%4:1+(tile*5+pixel*3)%15;
}
static void check_pattern_region(const uint32 *output, unsigned extra, int left, int right,
                                 unsigned size, unsigned format, bool flip, unsigned scroll)
{
  if(left>=right) return;
  int anchor=0;
  while(anchor>left) anchor-=(int)size;
  while(anchor+(int)size<=left) anchor+=(int)size;
  int next=anchor+(int)size, sample=left;
  for(int x=left;x<right;x++) {
    if(x==next) { sample=x; next+=(int)size; }
    uint32 expected=rgb(pattern(sample,format,flip,scroll));
    if(output[4+extra+x]!=expected) fprintf(stderr,"mosaic extra=%u size=%u format=%u flip=%u scroll=%u region=%d..%d x=%d sample=%d actual=%x expected=%x\n",extra,size,format,flip,scroll,left,right,x,sample,output[4+extra+x],expected);
    assert(output[4+extra+x]==expected);
  }
}
static void compose_patterned_mosaic(Ppu *ppu)
{
  uint32 output[kPpuXPixels+8]; unsigned cases=0;
  unsigned extras[]={0,1,32,96};
  for(unsigned e=0;e<4;e++)
   for(unsigned size=1;size<=16;size++)
    for(unsigned format=0;format<3;format++)
     for(unsigned flip=0;flip<=1;flip++)
      for(unsigned scroll=0;scroll<8;scroll++)
       for(unsigned window=0;window<=1;window++)
        for(unsigned tall=0;tall<=1;tall++) {
          unsigned extra=extras[e], layer=format==0?2:0;
          prepare(ppu,0,false,extra,false,0);
          ppu->screenEnabled[0]=1u<<layer;
          ppu->mosaicEnabled=1u<<layer; ppu->mosaicSize=size;
          ppu->bgLayer[layer].tileAdr=0x1000;
          ppu->bgLayer[layer].hScroll=scroll;
          if(format==2) {
            ppu->mode=7; ppu->m7matrix[0]=ppu->m7matrix[3]=256;
            ppu->m7matrix[6]=scroll; ppu->m7xFlip=flip;
            for(unsigned i=0;i<0x4000;i++) ppu->vram[i]=(i%32)+1;
            for(unsigned tile=0;tile<32;tile++) for(unsigned row=0;row<8;row++) for(unsigned x=0;x<8;x++) {
              unsigned address=(tile+1)*64+row*8+x, color=1+(tile*5+x*3)%15;
              ppu->vram[address]=(ppu->vram[address]&255)|(color<<8);
            }
          } else {
            for(unsigned i=0;i<0x400;i++) ppu->vram[i]=(i%32)|(flip?0x4000:0);
            for(unsigned tile=0;tile<32;tile++) for(unsigned row=0;row<8;row++) for(unsigned x=0;x<8;x++) {
              unsigned value=format==0?(tile+x)%4:1+(tile*5+x*3)%15;
              for(unsigned plane=0;plane<(format==0?2u:4u);plane++)
                if(value&(1u<<plane)) ppu->vram[0x1000+tile*(format==0?8:16)+row+(plane/2)*8]|=1u<<(7-x+(plane%2)*8);
            }
          }
          for(unsigned color=1;color<16;color++) ppu->cgram[color]=color|((31-color)<<5)|(color<<10);
          if(window) {
            ppu->screenWindowed[0]=1u<<layer;
            ppu->windowsel=2u<<(layer*4);
            ppu->window1left=0; ppu->window1right=255;
          }
          image_begin(ppu,output,extra,tall);
          ppu_runLine(ppu,1);
          if(format==0) {
            /* The original BG3/HUD layer stays 256 pixels wide in widescreen. */
            for(unsigned i=0;i<extra;i++) {
              assert(output[4+i]==0);
              assert(output[4+extra+256+i]==0);
            }
            if(window) {
              for(unsigned x=0;x<256;x++) assert(output[4+extra+x]==0);
            } else {
              check_pattern_region(output,extra,0,256,size,format,flip,scroll);
            }
          } else if(window) {
            check_pattern_region(output,extra,-(int)extra,0,size,format,flip,scroll);
            for(unsigned x=0;x<256;x++) assert(output[4+extra+x]==0);
            check_pattern_region(output,extra,256,256+extra,size,format,flip,scroll);
          } else {
            check_pattern_region(output,extra,-(int)extra,256+extra,size,format,flip,scroll);
          }
          guards(output,256+extra*2); cases++;
        }
  assert(cases==12288);
}
int main(void)
{
  Ppu *ppu = malloc(sizeof(*ppu)); assert(ppu);
  unsigned cases=0;
  for(unsigned extra=0;extra<=96;extra+=96) {
    for(unsigned large=0;large<=1;large++) {
      unsigned cap=large?17:32; /* 34 tile slivers or 32 objects, respectively. */
      for(unsigned count=0;count<=128;count++) {
       for(unsigned color=0;color<=15;color++) {
        for(unsigned unlimited=0;unlimited<=1;unlimited++) {
          prepare(ppu,count,large,extra,unlimited,color);
          assert(ppu_evaluateSprites(ppu,0)==(count>0));
          bool visible=(ppu->objBuffer.data[kPpuExtraLeftRight+64]&255)!=0;
          assert(visible==(count>0 && color>0 && (unlimited || count<=cap)));
          for(unsigned pixel=64;pixel<64+(large?16u:8u);pixel++) {
            assert((ppu->objBuffer.data[kPpuExtraLeftRight+pixel]&255)==(visible?0x80u+color:0));
          }
          /* A row below every object must remain empty even with limits off. */
          for(unsigned i=0;i<kPpuXPixels;i++) ppu->objBuffer.data[i]=0x500;
          assert(!ppu_evaluateSprites(ppu,32));
          for(unsigned i=0;i<kPpuXPixels;i++) assert(ppu->objBuffer.data[i]==0x500);
          compose_sprites(ppu,count,large,extra,unlimited,color);
          cases++;
        }
       }
      }
    }
  }
  assert(cases==16512);
  compose_backgrounds(ppu);
  compose_patterned_mosaic(ppu);
  free(ppu);
}
