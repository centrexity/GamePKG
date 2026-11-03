/* GamePKG portable original DMA save-state component. Apache-2.0.
 * The original 64-bit layout is 192 bytes; no host padding is serialized. */
#pragma once
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
enum { GAMEPKG_DMA_STATE_SIZE = 192 };
static inline void gamepkg_dma_put16(uint8_t *p,uint16_t value) {
  p[0]=(uint8_t)value;p[1]=(uint8_t)(value>>8);
}
static inline uint16_t gamepkg_dma_get16(const uint8_t *p) {
  return (uint16_t)(p[0]|(uint16_t)p[1]<<8);
}
static inline void gamepkg_dma_encode(const Dma *dma,uint8_t out[GAMEPKG_DMA_STATE_SIZE]) {
  memset(out,0,GAMEPKG_DMA_STATE_SIZE);
  for(unsigned i=0;i<8;i++) {
    const DmaChannel *c=&dma->channel[i];uint8_t *p=out+i*22;
    p[0]=c->bAdr;p[1]=c->aBank;p[2]=c->indBank;p[3]=c->repCount;
    gamepkg_dma_put16(p+4,c->aAdr);gamepkg_dma_put16(p+6,c->size);
    gamepkg_dma_put16(p+8,c->tableAdr);p[10]=c->unusedByte;
    p[11]=c->dmaActive;p[12]=c->hdmaActive;p[13]=c->mode;
    p[14]=c->fixed;p[15]=c->decrement;p[16]=c->indirect;p[17]=c->fromB;
    p[18]=c->unusedBit;p[19]=c->doTransfer;p[20]=c->terminated;p[21]=c->offIndex;
  }
  gamepkg_dma_put16(out+176,dma->hdmaTimer);
  for(unsigned i=0;i<4;i++)out[180+i]=(uint8_t)(dma->dmaTimer>>(i*8));
  out[184]=dma->dmaBusy;
}
static inline int gamepkg_dma_valid(const uint8_t in[GAMEPKG_DMA_STATE_SIZE]) {
  for(unsigned i=0;i<8;i++) {
    const uint8_t *p=in+i*22;
    if(p[13]>7 || p[21]>3)return 0;
    for(unsigned j=11;j<=20;j++)if(j!=13 && p[j]>1)return 0;
  }
  if(in[184]>1)return 0;
  return 1;
}
static inline int gamepkg_dma_decode(Dma *dma,const uint8_t in[GAMEPKG_DMA_STATE_SIZE]) {
  if(!gamepkg_dma_valid(in))return 0;
  /* Validate every indexed/boolean field before changing the live object. */
  Dma staged=*dma;
  for(unsigned i=0;i<8;i++) {
    DmaChannel *c=&staged.channel[i];const uint8_t *p=in+i*22;
    c->bAdr=p[0];c->aBank=p[1];c->indBank=p[2];c->repCount=p[3];
    c->aAdr=gamepkg_dma_get16(p+4);c->size=gamepkg_dma_get16(p+6);
    c->tableAdr=gamepkg_dma_get16(p+8);c->unusedByte=p[10];
    c->dmaActive=p[11];c->hdmaActive=p[12];c->mode=p[13];
    c->fixed=p[14];c->decrement=p[15];c->indirect=p[16];c->fromB=p[17];
    c->unusedBit=p[18];c->doTransfer=p[19];c->terminated=p[20];c->offIndex=p[21];
  }
  staged.hdmaTimer=gamepkg_dma_get16(in+176);staged.dmaTimer=0;
  for(unsigned i=0;i<4;i++)staged.dmaTimer|=(uint32_t)in[180+i]<<(i*8);
  staged.dmaBusy=in[184];
  *dma=staged;return 1;
}
static inline void gamepkg_dma_saveload(Dma *dma,SaveLoadFunc *func,void *ctx) {
  uint8_t bytes[GAMEPKG_DMA_STATE_SIZE];
  gamepkg_dma_encode(dma,bytes);
  func(ctx,bytes,sizeof(bytes));
  if(!gamepkg_dma_decode(dma,bytes))abort();
}
