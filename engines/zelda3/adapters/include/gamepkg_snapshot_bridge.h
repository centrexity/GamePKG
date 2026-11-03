/* Included in the reviewed runtime TU after its original static state helpers.
 * No file IO or replay log; all bytes stay in the engine-owned buffer. */
#pragma once
#include "gamepkg_snapshot.h"
#include "gamepkg_dma_state.h"
#include "gamepkg_dsp_state.h"
#include "gamepkg_ppu_state.h"
static const uint8_t gamepkg_snapshot_magic[8]={0x89,'G','P','Z','S','T',13,10};
static const uint8_t gamepkg_snapshot_profile[8]={'4','5','a','1','4','9','d','8'};
static uint32_t gamepkg_snapshot_get32(const uint8_t *p) {
  return p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;
}
static void gamepkg_snapshot_put32(uint8_t *p,uint32_t value) {
  for(unsigned i=0;i<4;i++)p[i]=(uint8_t)(value>>(i*8));
}
static uint32_t gamepkg_snapshot_crc32(const uint8_t *p,uint32_t length) {
  uint32_t crc=UINT32_MAX;
  for(uint32_t i=0;i<length;i++) {
    crc^=p[i];
    for(unsigned bit=0;bit<8;bit++)crc=(crc>>1)^((uint32_t)-(int32_t)(crc&1)&UINT32_C(0xedb88320));
  }
  return ~crc;
}
typedef struct { uint8_t *output;const uint8_t *input;uint32_t offset;int failed; } GamepkgSnapshotCursor;
static void gamepkg_snapshot_write(void *opaque,void *data,size_t length) {
  GamepkgSnapshotCursor *cursor=opaque;
  if(cursor->failed || cursor->offset>GAMEPKG_SNAPSHOT_PAYLOAD || length>GAMEPKG_SNAPSHOT_PAYLOAD-cursor->offset) {
    cursor->failed=1;return;
  }
  memcpy(cursor->output+cursor->offset,data,length);cursor->offset+=(uint32_t)length;
}
static void gamepkg_snapshot_read(void *opaque,void *data,size_t length) {
  GamepkgSnapshotCursor *cursor=opaque;
  if(cursor->failed || cursor->offset>GAMEPKG_SNAPSHOT_PAYLOAD || length>GAMEPKG_SNAPSHOT_PAYLOAD-cursor->offset)abort();
  memcpy(data,cursor->input+cursor->offset,length);cursor->offset+=(uint32_t)length;
}
static int gamepkg_snapshot_valid(const uint8_t *data,uint32_t length) {
  if(!data || length!=GAMEPKG_SNAPSHOT_SIZE || memcmp(data,gamepkg_snapshot_magic,8) ||
      gamepkg_snapshot_get32(data+8)!=1 || gamepkg_snapshot_get32(data+12)!=GAMEPKG_SNAPSHOT_PAYLOAD ||
      memcmp(data+16,gamepkg_snapshot_profile,8) || gamepkg_snapshot_get32(data+28)!=0)return 0;
  const uint8_t *payload=data+GAMEPKG_SNAPSHOT_HEADER;
  return gamepkg_snapshot_crc32(payload,GAMEPKG_SNAPSHOT_PAYLOAD)==gamepkg_snapshot_get32(data+24) &&
    gamepkg_dsp_valid(payload+GAMEPKG_SNAPSHOT_DSP) && gamepkg_dma_valid(payload+GAMEPKG_SNAPSHOT_DMA) &&
    gamepkg_ppu_valid(payload+GAMEPKG_SNAPSHOT_PPU);
}
int gamepkg_snapshot_capture(uint8_t *data,uint32_t capacity) {
  if(!data || capacity!=GAMEPKG_SNAPSHOT_SIZE)return -1;
  GamepkgSnapshotCursor cursor={.output=data+GAMEPKG_SNAPSHOT_HEADER};
  SaveSnesState(gamepkg_snapshot_write,&cursor);
  if(cursor.failed || cursor.offset!=GAMEPKG_SNAPSHOT_PAYLOAD)return -1;
  memcpy(data,gamepkg_snapshot_magic,8);gamepkg_snapshot_put32(data+8,1);
  gamepkg_snapshot_put32(data+12,GAMEPKG_SNAPSHOT_PAYLOAD);memcpy(data+16,gamepkg_snapshot_profile,8);
  gamepkg_snapshot_put32(data+24,gamepkg_snapshot_crc32(data+GAMEPKG_SNAPSHOT_HEADER,GAMEPKG_SNAPSHOT_PAYLOAD));
  gamepkg_snapshot_put32(data+28,0);
  return gamepkg_snapshot_valid(data,capacity)?0:-1;
}
int gamepkg_snapshot_restore(const uint8_t *data,uint32_t length) {
  if(!gamepkg_snapshot_valid(data,length))return -1;
  GamepkgSnapshotCursor cursor={.input=data+GAMEPKG_SNAPSHOT_HEADER};
  LoadSnesState(gamepkg_snapshot_read,&cursor);
  return cursor.offset==GAMEPKG_SNAPSHOT_PAYLOAD?0:-1;
}
