/* Original whole-state callback ordering with real component codecs.
 * Synthetic buses and observable audio/synchronization hooks; no game media. */
#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "snes/dsp.h"
#include "snes/dma.h"
#include "snes/ppu.h"
static uint8 g_ram[131072],sram_bytes[8192];
static Dsp dsp;
static Dma dma;
static Ppu ppu;
typedef struct {uint8 ram[65536];Dsp *dsp;} SyntheticPlayer;
static SyntheticPlayer player;
static struct {
  SyntheticPlayer *player;Dma *dma;Ppu *ppu;uint8 *sram,*ram;
} g_zenv={&player,&dma,&ppu,sram_bytes,g_ram};
static unsigned locked,saved_audio,restored_audio,synchronized;
static void ZeldaApuLock(void) {assert(!locked);locked=1;}
static void ZeldaApuUnlock(void) {assert(locked);locked=0;}
static void ZeldaSaveMusicStateToRam_Locked(void) {assert(locked);saved_audio++;}
static void ZeldaRestoreMusicAfterLoad_Locked(bool reset) {assert(locked && !reset);restored_audio++;}
static void EmuSynchronizeWholeState(void) {assert(!locked);synchronized++;}
#include GAMEPKG_SNAPSHOT_HELPERS
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
static uint8_t baseline[GAMEPKG_SNAPSHOT_SIZE],changed[GAMEPKG_SNAPSHOT_SIZE],current[GAMEPKG_SNAPSHOT_SIZE];
static void unchanged(void) {
  assert(gamepkg_snapshot_capture(current,sizeof(current))==0);
  assert(!memcmp(baseline,current,sizeof(current)));
}
static void rejected(uint32_t length) {
  unsigned restores=restored_audio,syncs=synchronized;
  assert(gamepkg_snapshot_restore(changed,length)==-1);
  assert(restored_audio==restores && synchronized==syncs && !locked);
  unchanged();
}
int main(void) {
  player.dsp=&dsp;dsp.apu_ram=player.ram;
  for(unsigned i=0;i<sizeof(g_ram);i++)g_ram[i]=(uint8)(i*37+11);
  for(unsigned i=0;i<sizeof(player.ram);i++)player.ram[i]=(uint8)(i*13+7);
  for(unsigned i=0;i<sizeof(sram_bytes);i++)sram_bytes[i]=(uint8)(i*23+5);
  for(unsigned i=0;i<32768;i++)ppu.vram[i]=(uint16_t)(i*31);
  for(unsigned i=0;i<256;i++)ppu.cgram[i]=(uint16_t)(i*47);
  assert(gamepkg_snapshot_crc32((const uint8_t *)"123456789",9)==UINT32_C(0xcbf43926));
  assert(gamepkg_snapshot_capture(baseline,sizeof(baseline))==0 && saved_audio==1 && !locked);
  assert(gamepkg_snapshot_get32(baseline+12)==275465);
  memset(g_ram,0,sizeof(g_ram));memset(player.ram,0,sizeof(player.ram));memset(sram_bytes,0,sizeof(sram_bytes));
  memset(ppu.vram,0,sizeof(ppu.vram));
  assert(gamepkg_snapshot_restore(baseline,sizeof(baseline))==0 && restored_audio==1 && synchronized==1);
  assert(dsp.apu_ram==player.ram && g_zenv.ram==g_ram);
  unchanged();
  const uint32_t lengths[]={0,1,31,32,275465,GAMEPKG_SNAPSHOT_SIZE-1,GAMEPKG_SNAPSHOT_SIZE+1,UINT32_MAX};
  memcpy(changed,baseline,sizeof(changed));
  for(unsigned i=0;i<sizeof(lengths)/sizeof(*lengths);i++)rejected(lengths[i]);
  assert(gamepkg_snapshot_restore(NULL,sizeof(changed))==-1);
  for(unsigned byte=0;byte<32;byte++) {
    memcpy(changed,baseline,sizeof(changed));changed[byte]^=0x80;rejected(sizeof(changed));
  }
  const unsigned positions[]={0,27,65563,65603,68627,68642,68834,136139,144331,144389,275464};
  for(unsigned i=0;i<sizeof(positions)/sizeof(*positions);i++) {
    memcpy(changed,baseline,sizeof(changed));changed[32+positions[i]]^=1;rejected(sizeof(changed));
  }
  const unsigned flags[]={GAMEPKG_SNAPSHOT_DSP+128+4,GAMEPKG_SNAPSHOT_DMA+11,GAMEPKG_SNAPSHOT_PPU+67138};
  for(unsigned i=0;i<sizeof(flags)/sizeof(*flags);i++) {
    memcpy(changed,baseline,sizeof(changed));changed[32+flags[i]]=2;
    gamepkg_snapshot_put32(changed+24,gamepkg_snapshot_crc32(changed+32,GAMEPKG_SNAPSHOT_PAYLOAD));
    rejected(sizeof(changed));
  }
  unsigned saves=saved_audio;
  assert(gamepkg_snapshot_capture(changed,sizeof(changed)-1)==-1 && saved_audio==saves);
  uint64_t digest=UINT64_C(14695981039346656037);
  for(unsigned i=0;i<sizeof(baseline);i++)digest=(digest^baseline[i])*UINT64_C(1099511628211);
  assert(digest==UINT64_C(0x5f8f5880092ac6e6));
  printf("Whole snapshot: %zu bytes, original callback/audio/sync ordering, CRC/profile/length/component rejection before loading, exact restore and digest %016llx; pointer width %zu.\n",
    sizeof(baseline),(unsigned long long)digest,sizeof(void*));
  return 0;
}
