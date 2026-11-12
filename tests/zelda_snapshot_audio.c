// Actual RAM-map, restore and echo setup functions with synthetic SPC state.
#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "src/types.h"
#include "src/spc_player.h"
#include "snes/dsp_regs.h"
static SpcPlayer player;
static Dsp dsp;
static struct { SpcPlayer *player; } g_zenv={&player};
static uint8 ports[4];
static uint8 writes[24];
static unsigned write_count,reset_count;
void SpcPlayer_Initialize(SpcPlayer *p) {assert(p==&player);reset_count++;}
static void Dsp_Write(SpcPlayer *p,uint8 reg,uint8 value) {
  assert(p==&player && write_count+2<=sizeof(writes));
  writes[write_count++]=reg;writes[write_count++]=value;p->dsp->ram[reg]=value;
}
#include GAMEPKG_SNAPSHOT_AUDIO_HELPERS
typedef struct { uint8 cache,echo,parameter,trace[24];unsigned count; } Observation;
static Observation observe(uint8 saved,uint8 inherited,uint8 requested,int restore) {
  player.dsp=&dsp;player.last_written_edl=inherited;player.timer_cycles=57;
  player.ram[0x4c]=(uint8)(saved^0x85);player.ram[0x4d]=(uint8)(saved^3);
  player.ram[0x48]=0x12;dsp.ram[EDL]=saved;
  for(unsigned i=0;i<4;i++)player.ram[0x410+i]=(uint8)(i+41);
  write_count=0;memset(writes,0,sizeof(writes));
  if(restore) {
    ZeldaRestoreMusicAfterLoad_Locked(false);
    assert(player.timer_cycles==0 && reset_count==0);
    for(unsigned i=0;i<4;i++)assert(ports[i]==i+41 && player.input_ports[i]==i+41);
  } else {
    SpcPlayer_CopyVariablesFromRam(&player);player.last_written_edl=saved;
  }
  SetupEchoParameter_EDL(&player,requested);
  Observation result={player.last_written_edl,player.echo_stored_time,player.echo_parameter_EDL,{0},write_count};
  memcpy(result.trace,writes,sizeof(writes));return result;
}
int main(void) {
  unsigned mismatches=0;
  for(unsigned saved=0;saved<256;saved++)for(unsigned inherited=0;inherited<256;inherited++) {
    for(unsigned change=0;change<2;change++) {
      uint8 requested=(uint8)(saved+change);
      Observation expected=observe((uint8)saved,(uint8)saved,requested,0);
      Observation actual=observe((uint8)saved,(uint8)inherited,requested,1);
      mismatches+=expected.cache!=actual.cache || expected.echo!=actual.echo || expected.parameter!=actual.parameter ||
        expected.count!=actual.count || memcmp(expected.trace,actual.trace,sizeof(writes))!=0;
    }
  }
#ifdef GAMEPKG_EXPECT_OLD
  assert(mismatches>0);printf("Old inherited echo cache rejected: %u mismatches\n",mismatches);
#else
  assert(mismatches==0);puts("Restored echo cache: 131072 exact register traces and port/timer checks passed");
#endif
  return 0;
}
