/* Production diagnostic tail: overflow, chunk schedules and fatal suffix. */
#include <assert.h>
#include <stdio.h>
#include "../engines/doom/adapters/diagnostics.h"
int main(void) {
  unsigned char source[17000];
  for(size_t i=0;i<sizeof(source);i++) source[i]=(unsigned char)(1+(i%126));
  const size_t chunks[]={1,2,21,511,2048,4094,4095,4096,9000};
  for(unsigned c=0;c<sizeof(chunks)/sizeof(*chunks);c++) {
    diagnostic_length=0;memset(diagnostics,0,sizeof(diagnostics));
    for(size_t offset=0;offset<sizeof(source);) {
      size_t count=chunks[c];if(count>sizeof(source)-offset)count=sizeof(source)-offset;
      gamepkg_diagnostics_append(source+offset,count);offset+=count;
      size_t expected=offset<4095?offset:4095;
      assert(diagnostic_length==expected && diagnostics[expected]==0);
      assert(!memcmp(diagnostics,source+offset-expected,expected));
    }
    const char fatal[]="R_FlatNumForName: PKGFLAT not found";
    gamepkg_diagnostics_append(fatal,sizeof(fatal)-1);
    assert(diagnostic_length==4095 && diagnostics[4095]==0);
    assert(!memcmp(diagnostics+4095-(sizeof(fatal)-1),fatal,sizeof(fatal)-1));
    char copy[4096];memcpy(copy,diagnostics,sizeof(copy));
    gamepkg_diagnostics_append(NULL,0);assert(!memcmp(copy,diagnostics,sizeof(copy)));
  }
  puts("DOOM diagnostic tail: bounded, null-terminated, chunk-invariant and fatal suffix retained.");
}
