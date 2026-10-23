/* Portable live SPC word/byte access. Apache-2.0. */
#pragma once
#include <stdint.h>
#include <stddef.h>
static inline uint16_t gamepkg_spc_read_le16(const uint8_t *data) {
  return (uint16_t)(data[0]|(uint16_t)data[1]<<8);
}
/* Canonical compiled banks end in exactly one two-byte zero-length marker.
   Destination addresses may wrap in the original 16-bit audio RAM. */
static inline int gamepkg_spc_bank_valid(const uint8_t *data,size_t length) {
  if (!data) return 0;
  size_t offset=0;
  while (length-offset>=2) {
    uint16_t count=gamepkg_spc_read_le16(data+offset);
    if (!count) return length-offset==2;
    if (length-offset<4 || count>length-offset-4) return 0;
    offset+=4+count;
  }
  return 0;
}
static inline uint16_t gamepkg_spc_read_ram(const uint8_t ram[65536],uint32_t address) {
  uint32_t low=address&65535u;
  return (uint16_t)(ram[low]|(uint16_t)ram[(low+1u)&65535u]<<8);
}
static inline uint8_t gamepkg_spc_high(uint16_t value) {return (uint8_t)(value>>8);}
static inline uint16_t gamepkg_spc_with_high(uint16_t value,uint8_t high) {
  return (uint16_t)((value&255u)|(uint16_t)high<<8);
}
