/* Bounded host-readable diagnostic tail; no filesystem or host writes. */
#ifndef GAMEPKG_DOOM_DIAGNOSTICS_H
#define GAMEPKG_DOOM_DIAGNOSTICS_H
#include <stdint.h>
#include <stddef.h>
#include <string.h>
static char diagnostics[4096];
static uint32_t diagnostic_length;
static inline void gamepkg_diagnostics_append(const void *data, size_t length) {
  const size_t capacity=sizeof(diagnostics)-1;
  const unsigned char *bytes=(const unsigned char *)data;
  if(!length) return;
  if(length>=capacity) {
    memmove(diagnostics,bytes+length-capacity,capacity);
    diagnostic_length=(uint32_t)capacity;
  } else {
    if(length>capacity-diagnostic_length) {
      size_t removed=length-(capacity-diagnostic_length);
      memmove(diagnostics,diagnostics+removed,diagnostic_length-removed);
      diagnostic_length-=(uint32_t)removed;
    }
    memmove(diagnostics+diagnostic_length,bytes,length);
    diagnostic_length+=(uint32_t)length;
  }
  diagnostics[diagnostic_length]=0;
}
#endif
