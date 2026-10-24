/* Host-owned Zelda engine snapshot interface. Apache-2.0. */
#pragma once
#include <stdint.h>
enum {
  GAMEPKG_SNAPSHOT_HEADER=32,
  GAMEPKG_SNAPSHOT_PAYLOAD=275465,
  GAMEPKG_SNAPSHOT_SIZE=GAMEPKG_SNAPSHOT_HEADER+GAMEPKG_SNAPSHOT_PAYLOAD,
  GAMEPKG_SNAPSHOT_DSP=65603,
  GAMEPKG_SNAPSHOT_DMA=68642,
  GAMEPKG_SNAPSHOT_PPU=68834
};
int gamepkg_snapshot_capture(uint8_t *data,uint32_t capacity);
int gamepkg_snapshot_restore(const uint8_t *data,uint32_t length);
