/* Production virtual storage tests; libc references use only synthetic temporary files. */
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
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*) == 4, "actual 32-bit process required");
#endif
static uint8_t *wad;
static uint32_t wad_length;
#include "../engines/doom/adapters/storage.h"
static void write_slot(unsigned slot, const void *bytes, size_t size) {
  char name[64]; snprintf(name,sizeof(name),"./.savegame/doomsav%u.dsg",slot);
  FILE *stream=__wrap_fopen("./.savegame/temp.dsg","wb"); assert(stream);
  assert(__wrap_fwrite(bytes,1,size,stream)==size); assert(!__wrap_fclose(stream));
  __wrap_remove(name); assert(!__wrap_rename("./.savegame/temp.dsg",name));
}
static void failed_temp_rewrite_cannot_replace_slot(void) {
  const uint8_t original[] = {1,2,3,4};
  write_slot(0,original,sizeof(original));
  FILE *stream=__wrap_fopen("./.savegame/temp.dsg","wb"); assert(stream);
  assert(__wrap_fwrite(original,1,sizeof(original),stream)==sizeof(original));
  assert(!__wrap_fclose(stream));
  assert(temp_length==sizeof(original));
  stream=__wrap_fopen("./.savegame/temp.dsg","wb"); assert(stream);
  const uint8_t changed[] = {9,8,7};
  assert(__wrap_fwrite(changed,1,sizeof(changed),stream)==sizeof(changed));
  assert(__wrap_fwrite(changed,1,SAVE_MAX,stream)==0);
  assert(__wrap_fclose(stream)==EOF);
  assert(__wrap_remove("./.savegame/doomsav0.dsg")==-1);
  assert(__wrap_rename("./.savegame/temp.dsg","./.savegame/doomsav0.dsg")==-1);
  assert(slot_lengths[0]==sizeof(original) && !memcmp(slots[0],original,sizeof(original)));
  assert(temp_length==0);
  storage_error=0;
  stream=__wrap_fopen("./.savegame/temp.dsg","wb"); assert(stream);
  assert(!__wrap_fclose(stream));
  assert(__wrap_rename("./.savegame/temp.dsg","./.savegame/doomsav0.dsg")==-1);
  write_slot(0,changed,sizeof(changed));
  assert(slot_lengths[0]==sizeof(changed) && !memcmp(slots[0],changed,sizeof(changed)));
}
static void reads_match_libc(void) {
  uint8_t payload[32];
  for (unsigned i=0;i<sizeof(payload);i++) payload[i]=(uint8_t)(i*7+3);
  const size_t counts[] = {0,1,2,3,7,40};
  for (unsigned length=1;length<=sizeof(payload);length++) {
    FILE *reference=tmpfile(); assert(reference);
    assert(fwrite(payload,1,length,reference)==length);
    wad=payload; wad_length=length;
    for (size_t item_size=0;item_size<=9;item_size++) {
      for (unsigned c=0;c<sizeof(counts)/sizeof(*counts);c++) {
        assert(!fseek(reference,0,SEEK_SET));
        FILE *virtual=__wrap_fopen("doom.wad","rb"); assert(virtual);
        for (unsigned attempt=0;attempt<2;attempt++) {
          uint8_t expected[400],actual[400];
          memset(expected,0xa5,sizeof(expected));memset(actual,0xa5,sizeof(actual));
          assert(fread(expected,item_size,counts[c],reference)==__wrap_fread(actual,item_size,counts[c],virtual));
          assert(!memcmp(expected,actual,sizeof(actual)));
          assert(ftell(reference)==__wrap_ftell(virtual));
          assert(!!feof(reference)==!!__wrap_feof(virtual));
          assert(!!ferror(reference)==!!__wrap_ferror(virtual));
        }
        uint8_t zero_expected=0xa5,zero_actual=0xa5;
        assert(fread(&zero_expected,0,1,reference)==__wrap_fread(&zero_actual,0,1,virtual));
        assert(fread(&zero_expected,1,0,reference)==__wrap_fread(&zero_actual,1,0,virtual));
        assert(zero_expected==0xa5 && zero_actual==0xa5);
        assert(!!feof(reference)==!!__wrap_feof(virtual));
        assert(!fseek(reference,0,SEEK_SET));
        assert(!__wrap_fseek(virtual,0,SEEK_SET));
        assert(!feof(reference) && !__wrap_feof(virtual));
        assert(fgetc(reference)==__wrap_fgetc(virtual));
        assert(!__wrap_fclose(virtual));
      }
    }
    assert(!fclose(reference));
  }
  wad=NULL;wad_length=0;
}
static void config_line_boundaries(void) {
  const unsigned lengths[]={0,1,79,99,178,253,254,255,256,1024};
  for (unsigned l=0;l<sizeof(lengths)/sizeof(*lengths);l++) {
    for (unsigned nul=0;nul<2;nul++) {
      unsigned length=lengths[l];
      uint8_t line[1025];memset(line,'x',length);
      if (nul && length) line[length/2]=0;
      FILE *stream=__wrap_fopen("gamepkg.cfg","wb");assert(stream);
      assert(__wrap_fwrite(line,1,length,stream)==length);
      assert(__wrap_fwrite("\nsfx_volume 7\n",1,14,stream)==14);
      assert(!__wrap_fclose(stream));
      stream=__wrap_fopen("gamepkg.cfg","rb");assert(stream);
      char name[80],value[100];
      int result=__wrap_fscanf(stream,"%79s %99[^\n]\n",name,value);
      if ((nul && length) || length>=255) assert(result==0);
      assert(__wrap_fscanf(stream,"%79s %99[^\n]\n",name,value)==2);
      assert(!strcmp(name,"sfx_volume") && !strcmp(value,"7"));
      assert(!__wrap_fclose(stream));
    }
  }
}
static void malformed_config_lines_preserve_following_settings(void) {
  const uint8_t malformed[] = "ignored \0 hidden\nsfx_volume 7\nmusic_volume 3";
  FILE *stream=__wrap_fopen("gamepkg.cfg","wb");assert(stream);
  assert(__wrap_fwrite(malformed,1,sizeof(malformed)-1,stream)==sizeof(malformed)-1);
  assert(!__wrap_fclose(stream));
  stream=__wrap_fopen("gamepkg.cfg","rb");assert(stream);
  char name[80],value[100];
  assert(__wrap_fscanf(stream,"%79s %99[^\n]\n",name,value)==0);
  assert(__wrap_fscanf(stream,"%79s %99[^\n]\n",name,value)==2);
  assert(!strcmp(name,"sfx_volume") && !strcmp(value,"7"));
  assert(__wrap_fscanf(stream,"%79s %99[^\n]\n",name,value)==2);
  assert(!strcmp(name,"music_volume") && !strcmp(value,"3"));
  assert(__wrap_fscanf(stream,"%79s %99[^\n]\n",name,value)==EOF);
  assert(!__wrap_fclose(stream));
}
static unsigned control_cases;
static void control_config_case(const char *name, const char *text, const char *expected) {
  control_cases++;
  char line[256];
  int length=snprintf(line,sizeof(line),"%s %s\nsfx_volume 7\n",name,text);
  assert(length>0 && (unsigned)length<sizeof(line));
  FILE *stream=__wrap_fopen("gamepkg.cfg","wb");assert(stream);
  assert(__wrap_fwrite(line,1,(size_t)length,stream)==(size_t)length);
  assert(!__wrap_fclose(stream));
  stream=__wrap_fopen("gamepkg.cfg","rb");assert(stream);
  char actual_name[80],value[100];
  int result=__wrap_fscanf(stream,"%79s %99[^\n]\n",actual_name,value);
  if(expected) {
    assert(result==2 && !strcmp(actual_name,name) && !strcmp(value,expected));
  } else {
    assert(result==0);
  }
  assert(__wrap_fscanf(stream,"%79s %99[^\n]\n",actual_name,value)==2);
  assert(!strcmp(actual_name,"sfx_volume") && !strcmp(value,"7"));
  assert(__wrap_fscanf(stream,"%79s %99[^\n]\n",actual_name,value)==EOF);
  assert(!__wrap_fclose(stream));
}
static void control_config_boundaries(void) {
  const char *mouse[]={"mouseb_fire","mouseb_strafe","mouseb_forward","mouseb_jump",
    "mouseb_strafeleft","mouseb_straferight","mouseb_use","mouseb_backward",
    "mouseb_prevweapon","mouseb_nextweapon"};
  const char *joystick[]={"joyb_fire","joyb_strafe","joyb_use","joyb_jump",
    "joyb_strafeleft","joyb_straferight","joyb_menu_activate","joyb_prevweapon","joyb_nextweapon"};
  const char *bad[]={"-2","-2147483648","2147483647","4294967295","0xfffffffe",
    "0x100000000","99999999999999999999999999999999","banana","0x","0Xz","\"\""};
  const char *inert_expected[]={"-2","-2147483648","2147483647",NULL,"-2",NULL,NULL,NULL,NULL,NULL,NULL};
  for(unsigned i=0;i<sizeof(mouse)/sizeof(*mouse);i++) {
    int indexed=i!=3 && i!=8 && i!=9; /* jump is unbound; weapon cycling compares. */
    for(unsigned j=0;j<sizeof(bad)/sizeof(*bad);j++)
      control_config_case(mouse[i],bad[j],indexed?NULL:inert_expected[j]);
    control_config_case(mouse[i],"8",indexed?NULL:"8");
    for(int value=-1;value<=7;value++) {
      char text[16];snprintf(text,sizeof(text),"%d",value);
      control_config_case(mouse[i],text,text);
    }
    control_config_case(mouse[i],"0xffffffff","-1");
    control_config_case(mouse[i],"0x7","7");
    control_config_case(mouse[i],"07","7");
    control_config_case(mouse[i],"0b111","0");
    control_config_case(mouse[i],"\" 7 \"","7");
    control_config_case(mouse[i],"3trailing","3");
  }
  for(unsigned i=0;i<sizeof(joystick)/sizeof(*joystick);i++) {
    int indexed=i<3 || i==4 || i==5;
    for(unsigned j=0;j<sizeof(bad)/sizeof(*bad);j++) {
      const char *expected=indexed?NULL:inert_expected[j];
      if(i==6 && j==2)expected=NULL; /* menu activation must not shift by INT_MAX. */
      control_config_case(joystick[i],bad[j],expected);
    }
    control_config_case(joystick[i],"20",indexed?NULL:"20");
    for(int value=-1;value<=19;value++) {
      char text[16];snprintf(text,sizeof(text),"%d",value);
      control_config_case(joystick[i],text,text);
    }
    control_config_case(joystick[i],"0xffffffff","-1");
  }
  control_config_case("joyb_speed","31","31");
  control_config_case("joyb_speed","20","20");
  control_config_case("joyb_speed","2147483647","2147483647");
  control_config_case("joyb_speed","0x7fffffff","2147483647");
  control_config_case("joyb_speed","-1","-1");
  control_config_case("joyb_speed","-2",NULL);
  control_config_case("key_fire","banana",NULL);
  control_config_case("key_fire","0x100000000",NULL);
  control_config_case("key_fire","2147483648",NULL);
  control_config_case("key_fire","0x1d","29");
  control_config_case("key_fire","0xffffffff","-1");
  control_config_case("key_fire","-2147483648","-2147483648");
  control_config_case("joyb_menu_activate","30","30");
  control_config_case("joyb_menu_activate","31",NULL);
  assert(control_cases==590);
  printf("Validated %u control configuration cases and following-setting preservation.\n",control_cases);
}
static void rendering_audio_config_boundaries(void) {
  unsigned before=control_cases;
  const struct { const char *name; int minimum,maximum; } fields[]={
    {"usegamma",0,4},{"screenblocks",3,11},{"sfx_volume",0,15},{"music_volume",0,15},
    {"mouse_sensitivity",0,30},{"snd_channels",0,16}
  };
  const char *bad[]={"banana","0x","\"\"","2147483648","-2147483649",
    "99999999999999999999999999999999","0xffffffff","-2147483648","2147483647"};
  for(unsigned i=0;i<sizeof(fields)/sizeof(*fields);i++) {
    for(int value=fields[i].minimum-1;value<=fields[i].maximum+1;value++) {
      char text[32];snprintf(text,sizeof(text),"%d",value);
      control_config_case(fields[i].name,text,
        value<fields[i].minimum || value>fields[i].maximum?NULL:text);
    }
    for(unsigned j=0;j<sizeof(bad)/sizeof(*bad);j++)control_config_case(fields[i].name,bad[j],NULL);
    char text[32],expected[32];snprintf(expected,sizeof(expected),"%d",fields[i].maximum);
    snprintf(text,sizeof(text),"0x%x",fields[i].maximum);control_config_case(fields[i].name,text,expected);
    snprintf(text,sizeof(text),"0%o",fields[i].maximum);control_config_case(fields[i].name,text,expected);
    snprintf(text,sizeof(text),"\" %d \"",fields[i].maximum);control_config_case(fields[i].name,text,expected);
  }
  const char *detail[]={"0","1","2","-1","-2147483648","2147483647","0xffffffff","0b1","\" 2 \""};
  for(unsigned i=0;i<sizeof(detail)/sizeof(*detail);i++)
    control_config_case("detaillevel",detail[i],i==0 || i==7?"0":"1");
  for(unsigned i=0;i<6;i++)control_config_case("detaillevel",bad[i],NULL);
  assert(control_cases-before==193);
  printf("Validated %u rendering/audio configuration cases.\n",control_cases-before);
}
int main(void) {
  control_config_boundaries();
  rendering_audio_config_boundaries();
  config_line_boundaries();
  malformed_config_lines_preserve_following_settings();
  reads_match_libc();
  failed_temp_rewrite_cannot_replace_slot();
  const char *attacks[]={"/etc/passwd","../doom.wad","./doom.wad","doom.wad/../gamepkg.cfg","gamepkg.cfg/anything","GAMEPKG.CFG","./.savegame/doomsav6.dsg","./.savegame/doomsav00.dsg","./.savegame/../gamepkg.cfg","C:\\Windows\\win.ini","//server/share"};
  for(unsigned i=0;i<sizeof(attacks)/sizeof(*attacks);i++) {
    assert(!__wrap_fopen(attacks[i],"rb")); assert(!__wrap_fopen(attacks[i],"wb"));
    assert(__wrap_rename("./.savegame/temp.dsg",attacks[i])==-1);
  }
  uint8_t patch[32]={ 'P','W','A','D' };
  put32(patch+4,1);put32(patch+8,16);put32(patch+16,12);put32(patch+20,4);
  memcpy(patch+24,"TESTLUMP",8);
  assert(register_patch(NULL,sizeof(patch))==-1);
  for(unsigned n=0;n<sizeof(patch);n++) assert(register_patch(patch,n)==-1);
  uint8_t damaged[32];memcpy(damaged,patch,sizeof(patch));put32(damaged+20,UINT32_MAX);
  assert(register_patch(damaged,sizeof(damaged))==-1 && patch_count==0);
  FILE *patch_files[PATCH_MAX];
  for(unsigned i=0;i<PATCH_MAX;i++) {
    assert(register_patch(patch,sizeof(patch))==0);
    for(unsigned mode=0;mode<=5;mode++) assert(register_patch_mode(i,mode)==0 && patch_modes[i]==mode);
    assert(register_patch_mode(i,6)==-1 && patch_modes[i]==5);
    assert(register_patch_mode(i,0)==0);
    assert(!__wrap_fopen(patch_names[i],"wb"));
    assert(!__wrap_access(patch_names[i],R_OK));assert(__wrap_access(patch_names[i],W_OK)==-1);
    patch_files[i]=__wrap_fopen(patch_names[i],"rb");assert(patch_files[i]);
    uint8_t copy[32];assert(__wrap_fread(copy,1,sizeof(copy),patch_files[i])==sizeof(copy));
    assert(!memcmp(copy,patch,sizeof(copy)));
    assert(__wrap_fwrite(copy,1,1,patch_files[i])==0);
    assert(__wrap_remove(patch_names[i])==-1);
  }
  assert(register_patch(patch,sizeof(patch))==-1 && patch_count==PATCH_MAX);
  wad=patch;wad_length=sizeof(patch);
  FILE *base=__wrap_fopen("doom.wad","rb");assert(base);
  FILE *spare=__wrap_fopen("gamepkg.cfg","w");assert(spare);
  assert(__wrap_fprintf(spare,"music_volume 8\n")>0);
  assert(!__wrap_fopen("gamepkg-extra.cfg","w") && errno==EMFILE);
  assert(!__wrap_fclose(spare));
  write_slot(0,"patch save",10);
  assert(!__wrap_fclose(base));
  for(unsigned i=0;i<PATCH_MAX;i++) assert(!__wrap_fclose(patch_files[i]));
  assert(register_patch_mode(PATCH_MAX,0)==-1);
  wad_files_sealed=1;
  assert(register_patch_mode(0,1)==-1 && patch_modes[0]==0);
  patch_count=0;
  assert(register_patch(patch,sizeof(patch))==-1);
  wad_files_sealed=0;
  const char *patch_attacks[]={"../patch0.wad","./patch0.wad","patch00.wad","patch6.wad","PATCH0.WAD","patch0.wad/../gamepkg.cfg"};
  for(unsigned i=0;i<sizeof(patch_attacks)/sizeof(*patch_attacks);i++) assert(!__wrap_fopen(patch_attacks[i],"rb"));
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
