/* Production DOOM virtual storage tests; no game media or filesystem calls. */
#include <assert.h>
#include <errno.h>
#include <limits.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
static uint8_t *wad;
static uint32_t wad_length;
#include "../engines/doom/adapters/storage.h"
static void write_slot(unsigned slot, const void *bytes, size_t size) {
  char name[64]; snprintf(name,sizeof(name),"./.savegame/doomsav%u.dsg",slot);
  FILE *stream=__wrap_fopen("./.savegame/temp.dsg","wb"); assert(stream);
  assert(__wrap_fwrite(bytes,1,size,stream)==size); assert(!__wrap_fclose(stream));
  __wrap_remove(name); assert(!__wrap_rename("./.savegame/temp.dsg",name));
}
int main(void) {
  const char *attacks[]={"/etc/passwd","../doom.wad","./doom.wad","doom.wad/../gamepkg.cfg","gamepkg.cfg/anything","GAMEPKG.CFG","./.savegame/doomsav6.dsg","./.savegame/doomsav00.dsg","./.savegame/../gamepkg.cfg","C:\\Windows\\win.ini","//server/share"};
  for(unsigned i=0;i<sizeof(attacks)/sizeof(*attacks);i++) {
    assert(!__wrap_fopen(attacks[i],"rb")); assert(!__wrap_fopen(attacks[i],"wb"));
    assert(__wrap_rename("./.savegame/temp.dsg",attacks[i])==-1);
  }
  assert(!restore_save(0));
  assert(__wrap_rename("./.savegame/temp.dsg","./.savegame/doomsav0.dsg")==-1);
  const char legacy[]="GAMEPKG legacy slot zero";
  memcpy(save_data,legacy,sizeof(legacy)); assert(!restore_save(sizeof(legacy)));
  assert(last_slot==0 && slot_lengths[0]==sizeof(legacy) && !memcmp(slots[0],legacy,sizeof(legacy)));
  for(unsigned i=0;i<6;i++) { uint8_t value=(uint8_t)i; write_slot(i,&value,1); }
  FILE *cfg=__wrap_fopen("gamepkg.cfg","w"); assert(cfg);
  assert(__wrap_fprintf(cfg,"%s %i\n","sfx_volume",7)>0);
  assert(__wrap_fprintf(cfg,"%s %i\n","music_volume",3)>0); assert(!__wrap_fclose(cfg));
  assert(dirty); dirty=0;
  cfg=__wrap_fopen("gamepkg.cfg","r"); assert(cfg);
  char name[80],value[100];
  assert(__wrap_fscanf(cfg,"%79s %99[^\n]\n",name,value)==2 && !strcmp(name,"sfx_volume") && !strcmp(value,"7"));
  assert(__wrap___isoc99_fscanf(cfg,"%79s %99[^\n]\n",name,value)==2 && !strcmp(name,"music_volume") && !strcmp(value,"3"));
  assert(__wrap_fscanf(cfg,"%s",name)==EOF); assert(!__wrap_fclose(cfg));
  cfg=__wrap_fopen("gamepkg.cfg","w"); assert(cfg);
  assert(__wrap_fprintf(cfg,"sfx_volume 7\nmusic_volume 3\n")>0); assert(!__wrap_fclose(cfg)); assert(!dirty);
  snapshot_save(); uint32_t size=save_length; assert(size>BUNDLE_HEADER && size<BUNDLE_MAX);
  assert(!restore_save(size) && last_slot==5);
  for(unsigned i=0;i<6;i++) assert(slot_lengths[i]==1 && slots[i][0]==i);
  uint8_t *good=malloc(size); assert(good); memcpy(good,save_data,size);
  for(uint32_t n=1;n<size;n++) {memcpy(save_data,good,size); assert(restore_save(n)==-1);}
  memcpy(save_data,good,size); save_data[4]='3'; assert(restore_save(size)==-1);
  memcpy(save_data,good,size); put32(save_data+8,6); assert(restore_save(size)==-1);
  memcpy(save_data,good,size); put32(save_data+12,UINT32_MAX); assert(restore_save(size)==-1);
  memcpy(save_data,good,size); put32(save_data+12+6*4,CONFIG_MAX+1); assert(restore_save(size)==-1);
  memcpy(save_data,good,size); save_data[size]=0; assert(restore_save(size+1)==-1);
  memcpy(save_data,good,size); assert(!restore_save(size)); free(good);
  uint8_t *maximum=malloc(SAVE_MAX); assert(maximum); memset(maximum,0x5a,SAVE_MAX);
  for(unsigned i=0;i<6;i++) write_slot(i,maximum,SAVE_MAX);
  snapshot_save(); assert(save_length>1024*1024 && save_length<BUNDLE_MAX);
  assert(!restore_save(save_length));
  FILE *stream=__wrap_fopen("./.savegame/doomsav0.dsg","rb"); assert(stream);
  assert(__wrap_fseek(stream,LONG_MIN,SEEK_CUR)==-1);
  assert(__wrap_fseek(stream,LONG_MAX,SEEK_END)==-1);
  assert(__wrap_fseek(stream,0,99)==-1); assert(!__wrap_fclose(stream));
  stream=__wrap_fopen("./.savegame/temp.dsg","wb"); assert(stream);
  assert(!__wrap_fopen("./.savegame/temp.dsg","wb"));
  assert(__wrap_fwrite(maximum,1,SAVE_MAX,stream)==SAVE_MAX);
  assert(!__wrap_fwrite(maximum,1,1,stream) && storage_error && __wrap_ferror(stream));
  assert(__wrap_fclose(stream)==EOF); free(maximum);
  uint32_t old_config_length=config_lengths[0];
  uint8_t old_config[CONFIG_MAX]; memcpy(old_config,configs[0],old_config_length);
  stream=__wrap_fopen("gamepkg.cfg","w"); assert(stream);
  uint8_t *large=calloc(1,CONFIG_MAX+1); assert(large);
  assert(!__wrap_fwrite(large,1,CONFIG_MAX+1,stream));
  assert(__wrap_fclose(stream)==EOF);
  assert(config_lengths[0]==old_config_length && !memcmp(configs[0],old_config,old_config_length));
  free(large);
  assert(!__wrap_fopen("doom.wad","wb")); assert(!__wrap_fopen("gamepkg.cfg","a"));
  return 0;
}
