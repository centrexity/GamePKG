/* GamePKG bounded DMX stereo mixer. GPL-2.0-or-later.
 * Panning/gain match the original DOOM quadratic separation and PCM scale:
 * https://github.com/id-Software/DOOM/blob/master/linuxdoom-1.10/i_sound.c
 */
#include <stdint.h>
typedef struct {
  const uint8_t *data;
  uint32_t length;
  uint64_t phase;
  uint32_t rate;
  int left, right, active;
} Channel;
static Channel channels[16];
static int16_t pcm[800 * 2];
static void channel_params(Channel *channel, int volume, int separation) {
  if (volume < 0) volume = 0;
  if (volume > 127) volume = 127;
  if (separation < 0) separation = 0;
  if (separation > 255) separation = 255;
  int a = separation + 1, b = separation - 256;
  channel->left = volume - volume * a * a / 65536;
  channel->right = volume - volume * b * b / 65536;
}
static int16_t clip_sample(int value) {
  return value > 32767 ? 32767 : value < -32768 ? -32768 : value;
}
static void mix(void) {
  for (int frame = 0; frame < 800; frame++) {
    int left = 0, right = 0;
    for (int c = 0; c < 16; c++) {
      Channel *channel = &channels[c];
      if (!channel->active) continue;
      uint64_t position = channel->phase / 48000;
      if (position >= channel->length || !channel->rate) {
        channel->active = 0;
        continue;
      }
      int sample = (int)channel->data[position] - 128;
      left += sample * channel->left * 256 / 127;
      right += sample * channel->right * 256 / 127;
      channel->phase += channel->rate;
    }
    pcm[frame * 2] = clip_sample(left);
    pcm[frame * 2 + 1] = clip_sample(right);
  }
}
