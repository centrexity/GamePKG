/* Original DMA callback with portable component codec; synthetic state only. */
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifndef GAMEPKG_DMA_SOURCE
#define GAMEPKG_DMA_SOURCE "../engines/zelda3/upstream/snes/dma.c"
#endif
#include GAMEPKG_DMA_SOURCE
#include "../engines/zelda3/adapters/include/gamepkg_dma_state.h"
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
static uint8_t saved[192];
static unsigned calls;
static void save(void *ctx,void *data,size_t size) {
  assert(ctx==saved && size==192);memcpy(saved,data,size);calls++;
}
static void load(void *ctx,void *data,size_t size) {
  assert(ctx==saved && size==192);memcpy(data,saved,size);calls++;
}
static Snes bus,other_bus;
static Dma specimen(void) {
  Dma dma;memset(&dma,0xa5,sizeof(dma));dma.snes=&bus;
  for(unsigned i=0;i<8;i++) dma.channel[i]=(DmaChannel){
    .bAdr=0x11,.aBank=0x22,.indBank=0x33,.repCount=0x44,
    .aAdr=0x6655,.size=0x8877,.tableAdr=0xaa99,.unusedByte=0xbb,
    .dmaActive=true,.hdmaActive=false,.mode=5,.fixed=false,.decrement=true,
    .indirect=false,.fromB=true,.unusedBit=false,.doTransfer=true,
    .terminated=false,.offIndex=3};
  dma.hdmaTimer=0xddcc;dma.dmaTimer=0x12345678;dma.dmaBusy=true;
  return dma;
}
int main(void) {
  const uint8_t channel[]={0x11,0x22,0x33,0x44,0x55,0x66,0x77,0x88,0x99,0xaa,0xbb,
    1,0,5,0,1,0,1,0,1,0,3};
  Dma dma=specimen();dma_saveload(&dma,save,saved);assert(calls==1);
  for(unsigned i=0;i<8;i++)assert(!memcmp(saved+i*22,channel,sizeof(channel)));
  const uint8_t tail[]={0xcc,0xdd,0,0,0x78,0x56,0x34,0x12,1,0,0,0,0,0,0,0};
  assert(!memcmp(saved+176,tail,sizeof(tail)));
  uint8_t golden[192];memcpy(golden,saved,sizeof(golden));
  Dma loaded={.snes=&other_bus};dma_saveload(&loaded,load,saved);
  assert(loaded.snes==&other_bus && loaded.hdmaTimer==0xddcc && loaded.dmaTimer==0x12345678 && loaded.dmaBusy);
  dma_saveload(&loaded,save,saved);assert(!memcmp(saved,golden,sizeof(golden)));
  for(unsigned i=0;i<8;i++)for(unsigned value=0;value<256;value++) {
    dma=specimen();dma.channel[i].bAdr=(uint8_t)value;dma_saveload(&dma,save,saved);
    for(unsigned j=0;j<192;j++)assert(saved[j]==(j==i*22?value:golden[j]));
  }
  unsigned rejected=0;
  for(unsigned byte=0;byte<192;byte++) {
    unsigned position=byte%22;
    int boolean=byte<176?(position==11 || position==12 || (position>=14 && position<=20)):byte==184;
    unsigned maximum=boolean?1:byte<176 && position==13?7:byte<176 && position==21?3:255;
    if(maximum!=255) for(unsigned value=0;value<=maximum;value++) {
      uint8_t valid[192],encoded[192];memcpy(valid,golden,sizeof(valid));valid[byte]=(uint8_t)value;
      Dma candidate=specimen();assert(gamepkg_dma_decode(&candidate,valid));
      gamepkg_dma_encode(&candidate,encoded);assert(!memcmp(valid,encoded,sizeof(valid)));
    }
    for(unsigned value=maximum+1;value<256;value++) {
      uint8_t damaged[192];memcpy(damaged,golden,sizeof(damaged));damaged[byte]=(uint8_t)value;
      Dma candidate=specimen();uint8_t before[sizeof(candidate)];memcpy(before,&candidate,sizeof(candidate));
      assert(!gamepkg_dma_decode(&candidate,damaged));
      assert(!memcmp(before,&candidate,sizeof(candidate)));rejected++;
    }
  }
  /* Historical padding is ignored on read and canonicalized on write. */
  memcpy(saved,golden,sizeof(saved));saved[178]=saved[179]=0xa5;
  memset(saved+185,0xa5,7);dma_saveload(&loaded,load,saved);
  dma_saveload(&loaded,save,saved);assert(!memcmp(saved,golden,sizeof(saved)));
  printf("DMA state: 192 canonical bytes, 2048 channel identity cases, %u invalid fields rejected without mutation; pointer width %zu.\n",rejected,sizeof(void*));
  return 0;
}
