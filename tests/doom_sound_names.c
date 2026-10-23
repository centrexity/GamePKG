#include <assert.h>
#include <stdio.h>
#include <string.h>
#include "../engines/doom/upstream/sounds.c"
#include "../engines/doom/adapters/sound_names.h"
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*) == 4, "actual 32-bit process required");
#endif
int main(void) {
  char name[9];
  assert(S_sfx[sfx_chgun].link == &S_sfx[sfx_pistol]);
  gamepkg_sound_lump_name(&S_sfx[sfx_chgun], 1, name);
  assert(!strcmp(name, "dspistol"));
  gamepkg_sound_lump_name(&S_sfx[sfx_chgun], 0, name);
  assert(!strcmp(name, "pistol"));
  for (int i = 1; i < NUMSFX; i++) for (int prefix = 0; prefix <= 1; prefix++) {
    const sfxinfo_t *expected = S_sfx[i].link ? S_sfx[i].link : &S_sfx[i];
    gamepkg_sound_lump_name(&S_sfx[i], prefix, name);
    assert(strlen(name) <= 8);
    if(prefix) {assert(name[0]=='d' && name[1]=='s');assert(!strncmp(name+2,expected->name,6));}
    else assert(!strncmp(name,expected->name,8));
  }
  sfxinfo_t custom = {.name="abcdefgh"};
  gamepkg_sound_lump_name(&custom,1,name);assert(!strcmp(name,"dsabcdef"));
  gamepkg_sound_lump_name(&custom,0,name);assert(!strcmp(name,"abcdefgh"));
  sfxinfo_t alias = {.name="missing",.link=&custom};
  gamepkg_sound_lump_name(&alias,0,name);assert(!strcmp(name,"abcdefgh"));
  return 0;
}
