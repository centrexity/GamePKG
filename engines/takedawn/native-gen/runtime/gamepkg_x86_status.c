/* NTSTATUS conversion adapted from Wine9.0 error.c/error.h.
 * Copyright 2000,2020 Alexandre Julliard
 * Copyright 2002 Andriy Palamarchuk
 * Copyright 2010 André Hentschel
 * LGPL-2.1-or-later; full notices/source/license in status/upstream/.
 * GamePKG replaces Windows types/debug/TEB dependencies with logical uint32_t
 * and an offline-generated constant map. Host TLS is never guest state. */
#include "gamepkg_x86_status.h"
#include "status/status_map.h"
uint32_t gamepkg_x86_status_to_dos(uint32_t status) {
  if (!status || (status & 0x20000000u))
    return status;
  if ((status & 0xf0000000u) == 0xd0000000u)
    status &= ~0x10000000u;
  uint32_t facility = status >> 16;
  if (facility == 0xc001u || facility == 0x8007u || facility == 0xc007u)
    return status & 0xffffu;
  uint32_t low = 0, high = 43;
  while (low < high) {
    uint32_t middle = low + (high - low) / 2;
    if (status < gamepkg_status_ranges[middle].start)
      high = middle;
    else if (status > gamepkg_status_ranges[middle].end)
      low = middle + 1;
    else
      return gamepkg_status_errors[gamepkg_status_ranges[middle].offset +
                                   status -
                                   gamepkg_status_ranges[middle].start];
  }
  return 317; /* ERROR_MR_MID_NOT_FOUND */
}
