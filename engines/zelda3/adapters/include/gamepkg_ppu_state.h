/* GamePKG portable original PPU state component. Apache-2.0. */
#pragma once
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
enum { GAMEPKG_PPU_STATE_SIZE=67305, GAMEPKG_PPU_CGRAM=65546, GAMEPKG_PPU_LAYERS=67138 };
static inline void gamepkg_ppu_put16(uint8_t *p,uint16_t value) {p[0]=(uint8_t)value;p[1]=(uint8_t)(value>>8);}
static inline uint16_t gamepkg_ppu_get16(const uint8_t *p) {return (uint16_t)(p[0]|(uint16_t)p[1]<<8);}
static inline void gamepkg_ppu_encode(const Ppu *ppu,uint8_t out[GAMEPKG_PPU_STATE_SIZE]) {
  memset(out,0,GAMEPKG_PPU_STATE_SIZE);
  for(unsigned i=0;i<32768;i++)gamepkg_ppu_put16(out+i*2,ppu->vram[i]);
  for(unsigned i=0;i<256;i++)gamepkg_ppu_put16(out+GAMEPKG_PPU_CGRAM+i*2,ppu->cgram[i]);
  for(unsigned i=0;i<4;i++) {
    uint8_t *p=out+GAMEPKG_PPU_LAYERS+i*12;
    p[0]=ppu->bgLayer[i].tilemapWider;p[1]=ppu->bgLayer[i].tilemapHigher;
    gamepkg_ppu_put16(p+2,ppu->bgLayer[i].tilemapAdr);
  }
}
static inline int gamepkg_ppu_valid(const uint8_t in[GAMEPKG_PPU_STATE_SIZE]) {
  for(unsigned i=0;i<4;i++) {
    const uint8_t *p=in+GAMEPKG_PPU_LAYERS+i*12;
    if(p[0]>1 || p[1]>1)return 0;
  }
  return 1;
}
static inline int gamepkg_ppu_decode(Ppu *ppu,const uint8_t in[GAMEPKG_PPU_STATE_SIZE]) {
  if(!gamepkg_ppu_valid(in))return 0;
  /* All constrained fields are valid before any video memory is overwritten.
   * Rendering pointers, scrolls and other original non-snapshot fields remain. */
  for(unsigned i=0;i<32768;i++)ppu->vram[i]=gamepkg_ppu_get16(in+i*2);
  for(unsigned i=0;i<256;i++)ppu->cgram[i]=gamepkg_ppu_get16(in+GAMEPKG_PPU_CGRAM+i*2);
  for(unsigned i=0;i<4;i++) {
    const uint8_t *p=in+GAMEPKG_PPU_LAYERS+i*12;
    ppu->bgLayer[i].tilemapWider=p[0];ppu->bgLayer[i].tilemapHigher=p[1];
    ppu->bgLayer[i].tilemapAdr=gamepkg_ppu_get16(p+2);
  }
  return 1;
}
static inline void gamepkg_ppu_saveload(Ppu *ppu,SaveLoadFunc *func,void *ctx) {
  uint8_t bytes[GAMEPKG_PPU_STATE_SIZE];gamepkg_ppu_encode(ppu,bytes);
  func(ctx,bytes,sizeof(bytes));if(!gamepkg_ppu_decode(ppu,bytes))abort();
}
