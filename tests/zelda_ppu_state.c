/* Actual PPU callback format with synthetic video memory and live render state. */
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifndef GAMEPKG_PPU_SOURCE
#define GAMEPKG_PPU_SOURCE "../engines/zelda3/upstream/snes/ppu.c"
#endif
#include GAMEPKG_PPU_SOURCE
#include "../engines/zelda3/adapters/include/gamepkg_ppu_state.h"
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
static Ppu original,loaded,candidate,expected;
static uint8_t rendering[32],before[sizeof(Ppu)],golden[67305];
static struct Stream {uint8_t bytes[67305];size_t offset;} stream;
static void save(void *ctx,void *data,size_t size) {
  struct Stream *s=ctx;assert(s==&stream && s->offset<=67305 && size<=67305-s->offset);
  memcpy(s->bytes+s->offset,data,size);s->offset+=size;
}
static void load(void *ctx,void *data,size_t size) {
  struct Stream *s=ctx;assert(s==&stream && s->offset<=67305 && size<=67305-s->offset);
  memcpy(data,s->bytes+s->offset,size);s->offset+=size;
}
static int ignored(unsigned byte) {
  if(byte<65536 || (byte>=65546 && byte<66058))return 0;
  for(unsigned i=0;i<4;i++)if(byte>=67138+i*12 && byte<67142+i*12)return 0;
  return 1;
}
static void prepare(void) {
  memset(&original,0,sizeof(original));memset(&loaded,0,sizeof(loaded));
  for(unsigned i=0;i<32768;i++)original.vram[i]=(uint16_t)((i*37)^0xa55a);
  for(unsigned i=0;i<256;i++)original.cgram[i]=(uint16_t)((i*7)^0x1234);
  for(unsigned i=0;i<4;i++) {
    original.bgLayer[i].tilemapWider=i&1;original.bgLayer[i].tilemapHigher=!(i&1);
    original.bgLayer[i].tilemapAdr=(uint16_t)(0x4567+i*0x111);
    loaded.bgLayer[i].hScroll=(uint16_t)(123+i);loaded.bgLayer[i].vScroll=(uint16_t)(456+i);
    loaded.bgLayer[i].tileAdr=(uint16_t)(789+i);
  }
  loaded.renderBuffer=rendering;loaded.renderPitch=4096;loaded.extraLeftRight=96;
  stream.offset=0;ppu_saveload(&original,save,&stream);assert(stream.offset==67305);
}
int main(int argc,char **argv) {
  prepare();
  if(argc==2 && !strcmp(argv[1],"--invalid-callback")) {
    stream.bytes[67138]=2;stream.offset=0;ppu_saveload(&loaded,load,&stream);
    puts("Invalid PPU flags were accepted.");return 0;
  }
  memcpy(golden,stream.bytes,sizeof(golden));
  for(unsigned i=0;i<67305;i++)if(ignored(i))assert(golden[i]==0);
  for(unsigned i=0;i<32768;i++) {
    unsigned value=(uint16_t)((i*37)^0xa55a);
    assert(golden[i*2]==(value&255) && golden[i*2+1]==(value>>8));
  }
  for(unsigned i=0;i<256;i++) {
    unsigned value=(uint16_t)((i*7)^0x1234);
    assert(golden[65546+i*2]==(value&255) && golden[65547+i*2]==(value>>8));
  }
  const uint8_t first_layer[]={0,1,0x67,0x45};assert(!memcmp(golden+67138,first_layer,4));
  expected=loaded;
  memcpy(expected.vram,original.vram,sizeof(expected.vram));memcpy(expected.cgram,original.cgram,sizeof(expected.cgram));
  for(unsigned i=0;i<4;i++) {
    expected.bgLayer[i].tilemapWider=original.bgLayer[i].tilemapWider;
    expected.bgLayer[i].tilemapHigher=original.bgLayer[i].tilemapHigher;
    expected.bgLayer[i].tilemapAdr=original.bgLayer[i].tilemapAdr;
  }
  stream.offset=0;ppu_saveload(&loaded,load,&stream);
  assert(stream.offset==67305 && !memcmp(&loaded,&expected,sizeof(loaded)));
  stream.offset=0;ppu_saveload(&loaded,save,&stream);assert(!memcmp(stream.bytes,golden,sizeof(golden)));
  unsigned rejected=0;
  for(unsigned layer=0;layer<4;layer++)for(unsigned flag=0;flag<2;flag++) {
    for(unsigned value=0;value<256;value++) {
      memcpy(stream.bytes,golden,sizeof(golden));stream.bytes[67138+layer*12+flag]=(uint8_t)value;
      candidate=loaded;memcpy(before,&candidate,sizeof(candidate));
      if(value>1) {
        assert(!gamepkg_ppu_decode(&candidate,stream.bytes));
        assert(!memcmp(before,&candidate,sizeof(candidate)));rejected++;
      } else {
        assert(gamepkg_ppu_decode(&candidate,stream.bytes));
        const BgLayer *b=&candidate.bgLayer[layer];
        assert((flag?b->tilemapHigher:b->tilemapWider)==(value!=0));
      }
    }
  }
  memcpy(stream.bytes,golden,sizeof(golden));
  for(unsigned i=0;i<67305;i++)if(ignored(i))stream.bytes[i]=0xa5;
  stream.offset=0;ppu_saveload(&loaded,load,&stream);stream.offset=0;ppu_saveload(&loaded,save,&stream);
  assert(!memcmp(stream.bytes,golden,sizeof(golden)));
  printf("PPU state: 67305 canonical bytes, all VRAM/CGRAM words, original field boundaries, retained rendering state and %u invalid flags rejected without mutation; pointer width %zu.\n",rejected,sizeof(void*));
  return 0;
}
