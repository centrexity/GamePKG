/* Synthetic-only original reader oracle. No ROM or game media. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include "util.h"
static uint8 g_ram[131072];
static void EmuSyncMemoryRegion(void *address,size_t length) {
  assert((uint8 *)address>=g_ram && (uint8 *)address<g_ram+sizeof(g_ram));
  assert(length==1);
}
void ByteArray_AppendByte(ByteArray *arr,uint8 value) {
  if(arr->size==arr->capacity) {
    size_t capacity=arr->capacity?arr->capacity*2:64;
    assert(capacity>arr->capacity);
    uint8 *grown=realloc(arr->data,capacity);assert(grown);
    arr->data=grown;arr->capacity=capacity;
  }
  arr->data[arr->size++]=value;
}
#include GAMEPKG_ORIGINAL_REPLAY
static uint32 read32(void) {
  uint8 bytes[4];assert(fread(bytes,1,4,stdin)==4);
  return (uint32)bytes[0]|(uint32)bytes[1]<<8|(uint32)bytes[2]<<16|(uint32)bytes[3]<<24;
}
static void write64(uint64_t value) {
  uint8 bytes[8];for(unsigned i=0;i<8;i++)bytes[i]=(uint8)(value>>(i*8));
  assert(fwrite(bytes,1,8,stdout)==8);
}
static void write32(uint32 value) {
  uint8 bytes[4];for(unsigned i=0;i<4;i++)bytes[i]=(uint8)(value>>(i*8));
  assert(fwrite(bytes,1,4,stdout)==4);
}
int main(int argc,char **argv) {
  if(argc==2 && !strcmp(argv[1],"--record")) {
    uint32 cases=read32();assert(cases<=64);
    for(uint32 test=0;test<cases;test++) {
      uint32 operations=read32();assert(operations<=4096);
      StateRecorder recorder={0};
      for(uint32 operation=0;operation<operations;operation++) {
        uint32 tag=read32();
        if(tag==0) {
          uint32 inputs=read32();assert(inputs<=0xfff);
          StateRecorder_Record(&recorder,(uint16)inputs);
        } else {
          assert(tag==1);
          uint32 address=read32(),length=read32();
          assert(address<sizeof(g_ram) && length>=1 && length<=1024);
          uint8 patch[1024];assert(fread(patch,1,length,stdin)==length);
          StateRecorder_RecordPatchByte(&recorder,address,patch,(int)length);
        }
      }
      assert(recorder.log.size<=UINT32_MAX);
      write32(recorder.total_frames);write32(recorder.frames_since_last);
      write32(recorder.last_inputs);write32((uint32)recorder.log.size);
      assert(fwrite(recorder.log.data,1,recorder.log.size,stdout)==recorder.log.size);
      fflush(stdout);free(recorder.log.data);
    }
    assert(fgetc(stdin)==EOF);
    return 0;
  }
  if(argc>1) {
    assert(argc==2 && !strcmp(argv[1],"--truncated-patch"));
    StateRecorder recorder={0};
    recorder.log.data=malloc(3);assert(recorder.log.data);
    memcpy(recorder.log.data,(uint8[]){0xc0,0,0},3);
    recorder.log.size=3;recorder.total_frames=1;recorder.replay_mode=true;
    puts("LEGACY_TRUNCATED_REPLAY_PATCH");fflush(stdout);
    (void)StateRecorder_ReadNextReplayState(&recorder);
    free(recorder.log.data);
    return 0;
  }
  uint32 cases=read32();assert(cases<=64);
  for(uint32 test=0;test<cases;test++) {
    uint32 frames=read32(),length=read32();assert(frames<=1024 && length<=65536);
    StateRecorder recorder={0};
    recorder.log.data=malloc(length?length:1);assert(recorder.log.data);
    recorder.log.size=length;recorder.total_frames=frames;recorder.replay_mode=true;
    assert(fread(recorder.log.data,1,length,stdin)==length);
    memset(g_ram,0,sizeof(g_ram));
    for(uint32 frame=0;frame<(frames?frames:1);frame++) {
      assert(recorder.replay_mode);
      uint16 inputs=StateRecorder_ReadNextReplayState(&recorder);
      uint8 encoded[2]={(uint8)inputs,(uint8)(inputs>>8)};
      assert(fwrite(encoded,1,2,stdout)==2);
      uint64_t hash=UINT64_C(14695981039346656037);
      for(unsigned i=0;i<sizeof(g_ram);i++){hash^=g_ram[i];hash*=UINT64_C(1099511628211);}
      write64(hash);
    }
    assert(!recorder.replay_mode);
    assert(fwrite(g_ram,1,sizeof(g_ram),stdout)==sizeof(g_ram));fflush(stdout);
    free(recorder.log.data);
  }
  assert(fgetc(stdin)==EOF);
  puts("ORIGINAL_REPLAY_COMPLETE");
}
