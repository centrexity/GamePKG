/* Exercise the production mixer without game assets or audio devices. */
#include <assert.h>
#include <string.h>
#include "../engines/doom/adapters/audio.h"
static uint8_t samples[100000];
static void start(int volume, int separation, uint32_t rate, uint32_t length) {
  memset(channels, 0, sizeof(channels));
  channels[0] = (Channel){.data = samples, .length = length,
                          .rate = rate, .active = 1};
  channel_params(&channels[0], volume, separation);
}
int main(void) {
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
