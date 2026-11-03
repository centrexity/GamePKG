/* Portable words in the original SPC RAM map. Apache-2.0.
 * The reviewed map uses size 2 for native uint16 fields; sizes 1 and 4
 * describe individual bytes and four-byte port arrays respectively. */
#pragma once
#include <stdint.h>
#include <stddef.h>
#include <string.h>
static inline void gamepkg_spc_to_ram(uint8_t *ram,const void *native,size_t length) {
  if(length==2) {
    uint16_t value;memcpy(&value,native,sizeof(value));
    ram[0]=(uint8_t)value;ram[1]=(uint8_t)(value>>8);
  } else memcpy(ram,native,length);
}
static inline void gamepkg_spc_from_ram(void *native,const uint8_t *ram,size_t length) {
  if(length==2) {
    uint16_t value=(uint16_t)(ram[0]|(uint16_t)ram[1]<<8);
    memcpy(native,&value,sizeof(value));
  } else memcpy(native,ram,length);
}
