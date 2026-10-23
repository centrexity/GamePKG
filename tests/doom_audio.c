/* Exercise the production mixer without game assets or audio devices. */
#include <assert.h>
#include <string.h>
#include "../engines/doom/adapters/audio.h"
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*) == 4, "actual 32-bit process required");
#endif
static uint8_t samples[100000];
static void start(int volume, int separation, uint32_t rate, uint32_t length) {
  memset(channels, 0, sizeof(channels));
  channels[0] = (Channel){.data = samples, .length = length,
                          .rate = rate, .active = 1};
  channel_params(&channels[0], volume, separation);
}
static void sample_framing(void) {
  uint8_t lump[209];
  for (uint32_t size = 0; size <= sizeof(lump); size++) {
    for (uint32_t length = 0; length <= 201; length++) {
      memset(lump, 0, sizeof(lump));
      lump[0] = 3; lump[2] = 0x80; lump[3] = 0xbb; /* 48 kHz */
      lump[4] = (uint8_t)length;
      memset(channels, 0, sizeof(channels));
      Channel previous;
      memcpy(&previous, &channels[0], sizeof(previous));
      int valid = size >= 8 && length > 48 && length <= size - 8;
      assert(channel_start(&channels[0], lump, size, 127, 0) == valid);
      if (!valid) { assert(!memcmp(&channels[0], &previous, sizeof(previous))); continue; }
      assert(channels[0].data == lump + 24 && channels[0].length == length - 32);
      /* Distinct loud padding must never enter the audible stream. */
      memset(lump + 8, 255, 16);
      memset(lump + 24, 128, length - 32);
      memset(lump + 8 + length - 16, 0, 16);
      lump[24] = 255;
      mix();
      assert(pcm[0] == 32512 && pcm[1] == 0);
      for (unsigned i = 2; i < sizeof(pcm)/sizeof(*pcm); i++) assert(pcm[i] == 0);
      assert(!channels[0].active);
    }
  }
  memset(lump, 0, sizeof(lump));
  lump[0] = 3; lump[2] = 1; lump[4] = 49;
  assert(channel_start(&channels[0], lump, 57, 127, 128));
  for (unsigned byte = 0; byte < 2; byte++) {
    lump[byte] ^= 0xff;
    assert(!channel_start(&channels[0], lump, sizeof(lump), 127, 128));
    lump[byte] ^= 0xff;
  }
  lump[2] = 0;
  assert(!channel_start(&channels[0], lump, sizeof(lump), 127, 128));
  lump[2] = 1;
  memset(lump + 4, 255, 4);
  assert(!channel_start(&channels[0], lump, sizeof(lump), 127, 128));
}
static void exact_sample_end(void) {
  const uint32_t rates[] = {1,8000,11025,16000,22050,24000,44100,48000,65535};
  const uint32_t boundaries[] = {799,800,801,1599,1600,1601};
  memset(samples,255,sizeof(samples));
  for (unsigned r=0;r<sizeof(rates)/sizeof(*rates);r++) {
    for (unsigned b=0;b<sizeof(boundaries)/sizeof(*boundaries);b++) {
      uint32_t length=(uint32_t)((uint64_t)rates[r]*boundaries[b]/48000);
      if (!length) length=1;
      uint64_t frames=((uint64_t)length*48000+rates[r]-1)/rates[r];
      start(127,0,rates[r],length);
      for (uint64_t block=0;block<(frames+799)/800+1;block++) {
        mix();
        for (unsigned frame=0;frame<800;frame++) {
          assert(pcm[frame*2] == (block*800+frame<frames ? 32512 : 0));
          assert(pcm[frame*2+1] == 0);
        }
        assert(!!channels[0].active == ((block+1)*800<frames));
      }
      assert(channels[0].phase == frames*rates[r]);
    }
  }
}
/* Distinct scheduled impulses expose omitted/crossed channels and early
 * completion independently of the mixer's accumulation loop. Expected PCM
 * values are the pinned original full-volume left/right/center levels. */
static void all_channels_finish_independently(void) {
  uint8_t voices[16][601];
  const int separation[] = {0,255,128};
  const int16_t levels[][2] = {{32512,0},{0,32512},{24320,24576}};
  memset(voices,128,sizeof(voices));
  memset(channels,0,sizeof(channels));
  for(unsigned c=0;c<16;c++) {
    unsigned width=c%2+1;
    unsigned position=c*40/width;
    voices[c][position]=255;
    channels[c]=(Channel){.data=voices[c],.length=position+1,
      .rate=48000/width,.active=1};
    channel_params(&channels[c],127,separation[c%3]);
  }
  mix();
  for(unsigned frame=0;frame<800;frame++) {
    unsigned voice=frame/40, offset=frame%40;
    for(unsigned side=0;side<2;side++) {
      int16_t expected=voice<16 && offset<voice%2+1?levels[voice%3][side]:0;
      assert(pcm[frame*2+side]==expected);
    }
  }
  for(unsigned c=0;c<16;c++) assert(!channels[c].active);
  mix();
  for(unsigned i=0;i<1600;i++) assert(!pcm[i]);
  /* Sixteen simultaneous opposite-polarity voices cancel before clipping.
   * Removing either member of the last pair exposes that exact voice. */
  for(unsigned c=0;c<16;c++) {
    memset(voices[c],c%2?1:255,sizeof(voices[c]));
    channels[c]=(Channel){.data=voices[c],.length=sizeof(voices[c]),
      .rate=1,.active=1};
    channel_params(&channels[c],127,0);
  }
  mix();
  for(unsigned i=0;i<1600;i++) assert(!pcm[i]);
  channels[14].active=0;
  mix();
  for(unsigned frame=0;frame<800;frame++) {
    assert(pcm[frame*2]==-32512 && !pcm[frame*2+1]);
  }
  channels[14].active=1;
  channels[15].active=0;
  mix();
  for(unsigned frame=0;frame<800;frame++) {
    assert(pcm[frame*2]==32512 && !pcm[frame*2+1]);
  }
}
int main(void) {
  all_channels_finish_independently();
  exact_sample_end();
  sample_framing();
  memset(samples, 255, sizeof(samples));
  start(127, 0, 48000, 100000);
  mix();
  assert(pcm[0] == 32512 && pcm[1] == 0);
  start(127, 255, 48000, 100000);
  mix();
  assert(pcm[0] == 0 && pcm[1] == 32512);
  start(127, 128, 48000, 100000);
  mix();
  assert(pcm[0] == 24320 && pcm[1] == 24576);
  channel_params(&channels[0], 0, 0);
  mix();
  assert(pcm[0] == 0 && pcm[1] == 0);
  start(1000, -1000, 48000, 100000);
  mix();
  assert(pcm[0] == 32512 && pcm[1] == 0);
  for (int c = 1; c < 16; c++) channels[c] = channels[0];
  mix();
  assert(pcm[0] == 32767 && pcm[1] == 0);
  memset(samples, 0, sizeof(samples));
  mix();
  assert(pcm[0] == -32768 && pcm[1] == 0);
  start(127, 0, 48000, 1);
  mix();
  assert(pcm[0] == -32768 && pcm[2] == 0 && !channels[0].active);
  start(127, 0, 0, 1);
  mix();
  assert(pcm[0] == 0 && !channels[0].active);
  memset(samples, 128, sizeof(samples));
  samples[90000] = 255;
  start(127, 0, 48000, 100000);
  channels[0].phase = UINT64_C(90000) * 48000;
  mix();
  assert(pcm[0] == 32512 && pcm[2] == 0);
  samples[0] = 255;
  start(127, 0, 24000, 1);
  mix();
  assert(pcm[0] == 32512 && pcm[2] == 32512 && pcm[4] == 0);
  return 0;
}
