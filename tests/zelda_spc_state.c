// Original SPC RAM map and actual production copy functions; synthetic state.
#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>
#include "src/types.h"
#include "src/spc_player.h"
#include "gamepkg_spc_state.h"
#include GAMEPKG_SPC_HELPERS
static SpcPlayer producer,reference,consumer,raw_consumer;
static Dsp dsp;
static uint8 expected[65536],ram_mask[65536],native_mask[sizeof(SpcPlayer)];
static uint8 producer_guard[sizeof(SpcPlayer)],consumer_guard[sizeof(SpcPlayer)];
static uint64_t digest=UINT64_C(14695981039346656037);
typedef struct {size_t native,wire,width;} Field;
static Field fields[1024];
static unsigned count;
static void add(size_t native,size_t wire,size_t width) {
  assert(count<1024 && native+width<=offsetof(SpcPlayer,ram) && wire+width<=65536);
  assert(width==1 || width==2 || width==4);
  for(size_t i=0;i<width;i++) {assert(!native_mask[native+i] && !ram_mask[wire+i]);native_mask[native+i]=1;ram_mask[wire+i]=1;}
  fields[count++]=(Field){native,wire,width};
}
static void guards(void) {
  const uint8 *a=(const uint8 *)&producer,*b=(const uint8 *)&consumer;
  for(size_t i=0;i<offsetof(SpcPlayer,ram);i++)if(!native_mask[i]) {
    assert(a[i]==producer_guard[i]);assert(b[i]==consumer_guard[i]);
  }
  for(size_t i=0;i<65536;i++) {
    assert(producer.ram[i]==expected[i]);
    assert(consumer.ram[i]==(ram_mask[i]?expected[i]:0x4e));
  }
}
int main(void) {
  const uint16_t probe=0x1234;int little=*(const uint8 *)&probe==0x34;
#ifdef GAMEPKG_EXPECT_BIG_ENDIAN
  assert(!little);
#endif
#ifdef GAMEPKG_TEST_32
  _Static_assert(sizeof(void *)==4,"not an actual 32-bit fixture");
#endif
  memset(&producer,0xa5,sizeof(producer));memset(&consumer,0x5a,sizeof(consumer));
  producer.dsp=consumer.dsp=&dsp;producer.reg_write_history=consumer.reg_write_history=NULL;
  producer.timer_cycles=37;consumer.timer_cycles=51;
  for(unsigned i=0;i<8;i++) {producer.channel[i].index=(uint8)i;consumer.channel[i].index=(uint8)i;}
  memset(producer.ram,0x6d,sizeof(producer.ram));memset(consumer.ram,0x4e,sizeof(consumer.ram));
  memcpy(&reference,&producer,sizeof(reference));memcpy(&raw_consumer,&consumer,sizeof(raw_consumer));
  memcpy(producer_guard,&producer,sizeof(producer));memcpy(consumer_guard,&consumer,sizeof(consumer));
  memset(expected,0x6d,sizeof(expected));
  for(unsigned channel=0;channel<8;channel++)for(size_t i=0;i<sizeof(kChannel_Maps)/sizeof(*kChannel_Maps);i++) {
    const MemMap *m=&kChannel_Maps[i];
    add(offsetof(SpcPlayer,channel)+channel*sizeof(Channel)+m->off,(m->org_off&0x7fff)+channel*2,m->org_off&0x8000?2:1);
  }
  for(size_t i=0;i<sizeof(kSpcPlayer_Maps)/sizeof(*kSpcPlayer_Maps);i++) {
    const MemMapSized *m=&kSpcPlayer_Maps[i];add(m->off,m->org_off,m->size);
  }
  unsigned legacy_mismatches=0;
  for(unsigned seed=0;seed<65536;seed++) {
    for(unsigned i=0;i<count;i++) {
      Field f=fields[i];uint8 *a=(uint8 *)&producer+f.native,*b=(uint8 *)&reference+f.native;
      if(f.width==2) {
        uint16_t value=(uint16_t)(seed^(i*257u));memcpy(a,&value,2);memcpy(b,&value,2);
        expected[f.wire]=(uint8)value;expected[f.wire+1]=(uint8)(value>>8);
      } else for(size_t byte=0;byte<f.width;byte++) {
        a[byte]=b[byte]=expected[f.wire+byte]=(uint8)(seed+i*43u+byte*17u);
      }
    }
    SpcPlayer_CopyVariablesToRam(&producer);reference_to(&reference);
    for(unsigned i=0;i<count;i++) {
      Field f=fields[i];assert(memcmp(producer.ram+f.wire,expected+f.wire,f.width)==0);
      int same=memcmp(reference.ram+f.wire,expected+f.wire,f.width)==0;
      if(little)assert(same);else legacy_mismatches+=!same;
      memcpy(consumer.ram+f.wire,expected+f.wire,f.width);memcpy(raw_consumer.ram+f.wire,expected+f.wire,f.width);
      for(size_t byte=0;byte<f.width;byte++) {digest^=producer.ram[f.wire+byte];digest*=UINT64_C(1099511628211);}
    }
    SpcPlayer_CopyVariablesFromRam(&consumer);reference_from(&raw_consumer);
    for(unsigned i=0;i<count;i++) {
      Field f=fields[i];const uint8 *expected_native=(const uint8 *)&reference+f.native;
      assert(memcmp((const uint8 *)&producer+f.native,expected_native,f.width)==0);
      assert(memcmp((const uint8 *)&consumer+f.native,expected_native,f.width)==0);
      int same=memcmp((const uint8 *)&raw_consumer+f.native,expected_native,f.width)==0;
      if(little)assert(same);else legacy_mismatches+=!same;
    }
    if(seed==0 || seed==1 || seed==127 || seed==32768 || seed==65535)guards();
  }
  assert(little || legacy_mismatches>0);
  assert(count==486 && digest==UINT64_C(0x4cc3066ee2043b25));
  printf("SPC original RAM map: %u fields, all 65536 word values, byte/port arrays, exact decode, unmapped bytes/pointers/timer preserved; digest %016llx; pointer width %zu; %s endian; legacy mismatches %u.\n",
    count,(unsigned long long)digest,sizeof(void *),little?"little":"big",legacy_mismatches);
  return 0;
}
