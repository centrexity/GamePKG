/* GamePKG private DOOM files and versioned save bundle. GPL-2.0-or-later.
 * Exact virtual names only; no operation reaches a host file descriptor.
 */
#include <ctype.h>
#define SAVE_MAX (512 * 1024)
#define CONFIG_MAX (64 * 1024)
#define BUNDLE_MAX (4 * 1024 * 1024)
#define BUNDLE_HEADER 44
static uint8_t save_data[BUNDLE_MAX], temp_data[SAVE_MAX];
static uint8_t slots[6][SAVE_MAX], configs[2][CONFIG_MAX], config_stage[2][CONFIG_MAX];
static uint32_t save_length, temp_length, slot_lengths[6], config_lengths[2];
static int dirty, storage_error, last_slot = -1;
static const uint8_t bundle_magic[8] = {0x89,'G','P','D','2',13,10,26};
static uint32_t get32(const uint8_t *p) {
  return (uint32_t)p[0] | (uint32_t)p[1]<<8 | (uint32_t)p[2]<<16 | (uint32_t)p[3]<<24;
}
/* Six persistent patches + IWAD leave one of the unchanged eight handles
 * available for original configuration/save operations. */
#define PATCH_MAX 6
static uint8_t *patch_data[PATCH_MAX];
static uint32_t patch_lengths[PATCH_MAX], patch_count;
static int wad_files_sealed;
static uint32_t patch_modes[PATCH_MAX];
static const char *patch_names[PATCH_MAX] = {"patch0.wad","patch1.wad","patch2.wad","patch3.wad","patch4.wad","patch5.wad"};
static int register_patch(uint8_t *data, uint32_t length) {
  if(wad_files_sealed || patch_count>=PATCH_MAX || !data || length<12 || length>64u*1024*1024 || memcmp(data,"PWAD",4)) return -1;
  uint32_t count=get32(data+4), offset=get32(data+8);
  if(count>65536 || offset>length || count>(length-offset)/16) return -1;
  for(uint32_t i=0;i<count;i++) {
    const uint8_t *entry=data+offset+i*16;
    uint32_t start=get32(entry), size=get32(entry+4);
    if(start>length || size>length-start) return -1;
  }
  patch_data[patch_count]=data; patch_lengths[patch_count++]=length; return 0;
}
static int register_patch_mode(uint32_t index, uint32_t mode) {
  if(wad_files_sealed || index>=patch_count || mode>5) return -1;
  patch_modes[index]=mode;return 0;
}
static void put32(uint8_t *p, uint32_t n) {
  for (unsigned i=0;i<4;i++) p[i]=(uint8_t)(n>>(i*8));
}
static int restore_save(uint32_t length) {
  if (length > BUNDLE_MAX) return -1;
  memset(slot_lengths,0,sizeof(slot_lengths)); memset(config_lengths,0,sizeof(config_lengths));
  last_slot=-1; temp_length=0;
  if (!length) return 0;
  if (length < 8 || memcmp(save_data,bundle_magic,8)) {
    /* Never reinterpret a truncated or future binary bundle as a legacy .dsg. */
    if (save_data[0]==0x89 || length > SAVE_MAX) return -1;
    memcpy(slots[0],save_data,length); slot_lengths[0]=length; last_slot=0;
    return 0;
  }
  if (length < BUNDLE_HEADER) return -1;
  uint32_t selected=get32(save_data+8), offset=BUNDLE_HEADER;
  if (selected!=UINT32_MAX && selected>=6) return -1;
  /* Validate the whole bundle before changing any slot contents. */
  for (unsigned i=0;i<8;i++) {
    uint32_t n=get32(save_data+12+i*4), cap=i<6?SAVE_MAX:CONFIG_MAX;
    if (n>cap || n>length-offset) return -1;
    offset+=n;
  }
  if (offset!=length || (selected!=UINT32_MAX && !get32(save_data+12+selected*4))) return -1;
  offset=BUNDLE_HEADER;
  for (unsigned i=0;i<8;i++) {
    uint32_t n=get32(save_data+12+i*4);
    memcpy(i<6?slots[i]:configs[i-6],save_data+offset,n);
    if (i<6) slot_lengths[i]=n; else config_lengths[i-6]=n;
    offset+=n;
  }
  last_slot=selected==UINT32_MAX?-1:(int)selected;
  return 0;
}
static void snapshot_save(void) {
  memcpy(save_data,bundle_magic,8); put32(save_data+8,last_slot<0?UINT32_MAX:(uint32_t)last_slot);
  uint32_t offset=BUNDLE_HEADER;
  for (unsigned i=0;i<8;i++) {
    uint32_t n=i<6?slot_lengths[i]:config_lengths[i-6];
    put32(save_data+12+i*4,n);
    memcpy(save_data+offset,i<6?slots[i]:configs[i-6],n); offset+=n;
  }
  save_length=offset;
}
typedef struct {
  uint8_t *data;
  uint32_t size, capacity, pos;
  int used, writing, kind, error, eof;
} File;
static File files[8];
static File *file(FILE *stream) {
  for (unsigned i=0;i<8;i++) if (stream==(FILE *)&files[i] && files[i].used) return &files[i];
  return NULL;
}
static int kind(const char *path) {
  if (!path) return 0;
  if (!strcmp(path,"doom.wad")) return 1;
  for(unsigned i=0;i<patch_count;i++) if(!strcmp(path,patch_names[i])) return 11+(int)i;
  if (!strcmp(path,"./.savegame/temp.dsg")) return 2;
  const char *names[6]={"./.savegame/doomsav0.dsg","./.savegame/doomsav1.dsg","./.savegame/doomsav2.dsg","./.savegame/doomsav3.dsg","./.savegame/doomsav4.dsg","./.savegame/doomsav5.dsg"};
  for (unsigned i=0;i<6;i++) if (!strcmp(path,names[i])) return (int)i+3;
  if (!strcmp(path,"gamepkg.cfg")) return 9;
  if (!strcmp(path,"gamepkg-extra.cfg")) return 10;
  return 0;
}
FILE *__wrap_fopen(const char *path,const char *mode) {
  int k=kind(path), writing=mode && (!strcmp(mode,"w") || !strcmp(mode,"wb"));
  int reading=mode && (!strcmp(mode,"r") || !strcmp(mode,"rb"));
  if (!k || (!writing&&!reading) || ((k==1||k>=11||(k>=3&&k<=8))&&writing)) { errno=EACCES; return NULL; }
  uint32_t length=k>=11?patch_lengths[k-11]:k==1?wad_length:k==2?temp_length:k<=8?slot_lengths[k-3]:config_lengths[k-9];
  if (!writing && !length) { errno=ENOENT; return NULL; }
  for (unsigned i=0;i<8;i++) if (files[i].used && files[i].kind==k && (files[i].writing||writing)) {
    errno=EBUSY; return NULL;
  }
  for (unsigned i=0;i<8;i++) if (!files[i].used) {
    File *f=&files[i];
    *f=(File){.used=1,.kind=k,.writing=writing,.size=writing?0:length};
    f->data=k>=11?patch_data[k-11]:k==1?wad:k==2?temp_data:k<=8?slots[k-3]:writing?config_stage[k-9]:configs[k-9];
    f->capacity=k>=11?patch_lengths[k-11]:k==1?wad_length:k<=8?SAVE_MAX:CONFIG_MAX;
    if (writing && k==2) temp_length=0; /* wb truncates before the first write. */
    return (FILE *)f;
  }
  errno=EMFILE; return NULL;
}
int __wrap_fclose(FILE *stream) {
  File *f=file(stream); if (!f) return EOF;
  int result=f->error?EOF:0;
  if (f->writing && !f->error) {
    if (f->kind==2) temp_length=f->size;
    else if (f->kind<=8) { slot_lengths[f->kind-3]=f->size; dirty=1; last_slot=f->kind-3; }
    else {
      unsigned i=f->kind-9;
      if (config_lengths[i]!=f->size || memcmp(configs[i],f->data,f->size)) {
        memcpy(configs[i],f->data,f->size); config_lengths[i]=f->size; dirty=1;
      }
    }
  }
  f->used=0; return result;
}
size_t __wrap_fread(void *out,size_t size,size_t count,FILE *stream) {
  File *f=file(stream); if (!f||f->writing||!size||!count||count>SIZE_MAX/size) return 0;
  size_t bytes=size*count, available=f->size-f->pos;
  if (bytes>available) { bytes=available; f->eof=1; }
  if (bytes) memcpy(out,f->data+f->pos,bytes);
  f->pos+=(uint32_t)bytes; return bytes/size;
}
size_t __wrap_fwrite(const void *in,size_t size,size_t count,FILE *stream) {
  File *f=file(stream); if (!f||!f->writing||!size||count>SIZE_MAX/size) return 0;
  size_t n=size*count;
  if (n>f->capacity-f->pos) { f->error=1; storage_error=1; errno=ENOSPC; return 0; }
  memcpy(f->data+f->pos,in,n); f->pos+=(uint32_t)n;
  if (f->pos>f->size) f->size=f->pos;
  return count;
}
int __wrap_fseek(FILE *stream,long offset,int whence) {
  File *f=file(stream); if (!f || (whence!=SEEK_SET&&whence!=SEEK_CUR&&whence!=SEEK_END)) return -1;
  int64_t base=whence==SEEK_CUR?f->pos:whence==SEEK_END?f->size:0;
  if (offset < -base || offset > (int64_t)f->size-base) return -1;
  f->pos=(uint32_t)(base+offset); f->eof=0; return 0;
}
long __wrap_ftell(FILE *stream) { File *f=file(stream); return f?(long)f->pos:-1; }
int __wrap_feof(FILE *stream) { File *f=file(stream); return !f||f->eof; }
int __wrap_ferror(FILE *stream) { File *f=file(stream); return !f||f->error; }
int __wrap_fflush(FILE *stream) { return file(stream)?0:EOF; }
int __wrap_fgetc(FILE *stream) { uint8_t c; return __wrap_fread(&c,1,1,stream)==1?c:EOF; }
int __wrap_fputc(int c,FILE *stream) { uint8_t value=(uint8_t)c; return __wrap_fwrite(&value,1,1,stream)==1?value:EOF; }
int __wrap_fprintf(FILE *stream,const char *format,...) {
  File *f=file(stream); if (!f) return 0;
  char text[4096]; va_list args; va_start(args,format);
  int n=vsnprintf(text,sizeof(text),format,args); va_end(args);
  if (n<0 || (unsigned)n>=sizeof(text) || __wrap_fwrite(text,1,(size_t)n,stream)!=(size_t)n) {
    f->error=1; storage_error=1; return -1;
  }
  return n;
}
char *__wrap_fgets(char *out,int n,FILE *stream) {
  if (n<=1 || !file(stream)) return NULL;
  int i=0,c; while (i<n-1 && (c=__wrap_fgetc(stream))!=EOF) { out[i++]=(char)c; if (c=='\n') break; }
  out[i]=0; return i?out:NULL;
}
static int numeric_config_value(const char *name,char *value) {
  int mouse=!strncmp(name,"mouseb_",7), joystick=!strncmp(name,"joyb_",5);
  int gamma=!strcmp(name,"usegamma"), screen=!strcmp(name,"screenblocks");
  int volume=!strcmp(name,"sfx_volume") || !strcmp(name,"music_volume");
  int detail=!strcmp(name,"detaillevel");
  int sensitivity=!strcmp(name,"mouse_sensitivity");
  int sound_channels=!strcmp(name,"snd_channels");
  if(!mouse && !joystick && strncmp(name,"key_",4) && !gamma && !screen && !volume && !detail && !sensitivity && !sound_channels) return 1;
  char working[100]; size_t length=strlen(value);
  memcpy(working,value,length+1);
  while(length && !isprint((unsigned char)working[length-1])) working[--length]=0;
  char *start=working;
  if(length>=2 && working[0]=='"' && working[length-1]=='"') {
    working[length-1]=0; start++;
  }
  const char *digits=start;
  while(isspace((unsigned char)*digits)) digits++;
  if(*digits=='+' || *digits=='-') digits++;
  int hexadecimal=digits[0]=='0' && (digits[1]=='x' || digits[1]=='X');
  int base=hexadecimal?16:digits[0]=='0'?8:10;
  char *end;
  int saved_errno=errno;
  errno=0;
  long long parsed=strtoll(start,&end,base);
  int invalid=errno==ERANGE || end==start;
  errno=saved_errno;
  if(invalid) return 0;
  if(hexadecimal && end<=digits+2) return 0;
  /* Preserve the original signed 32-bit hexadecimal bit patterns without
   * passing an out-of-range conversion into its int-based sscanf parser. */
  if(hexadecimal && parsed>INT32_MAX && parsed<=UINT32_MAX) parsed-=4294967296LL;
  if(parsed<INT32_MIN || parsed>INT32_MAX) return 0;
  if(gamma && (parsed<0 || parsed>4)) return 0;
  if(screen && (parsed<3 || parsed>11)) return 0;
  if(volume && (parsed<0 || parsed>15)) return 0;
  /* Preserve extended original sensitivity values above the menu's nine;
   * the original config documents 31+ as crashing the Options thermometer. */
  if(sensitivity && (parsed<0 || parsed>30)) return 0;
  /* Match audio.h's existing sixteen physical mixer channels. Zero retains
   * the original no-effects setting; invalid allocation sizes use defaults. */
  if(sound_channels && (parsed<0 || parsed>16)) return 0;
  /* The original setting documents every nonzero value as low detail.
   * Its renderer requires that flag to be a canonical zero/one shift. */
  if(detail) parsed=parsed!=0;
  const char *mouse_indexes[]={"mouseb_fire","mouseb_strafe","mouseb_forward",
    "mouseb_strafeleft","mouseb_straferight","mouseb_use","mouseb_backward"};
  const char *joystick_indexes[]={"joyb_fire","joyb_strafe","joyb_use",
    "joyb_strafeleft","joyb_straferight"};
  for(unsigned i=0;i<sizeof(mouse_indexes)/sizeof(*mouse_indexes);i++) {
    if(!strcmp(name,mouse_indexes[i]) && (parsed < -1 || parsed >= 8)) return 0;
  }
  for(unsigned i=0;i<sizeof(joystick_indexes)/sizeof(*joystick_indexes);i++) {
    if(!strcmp(name,joystick_indexes[i]) && (parsed < -1 || parsed >= 20)) return 0;
  }
  if(!strcmp(name,"joyb_speed") && parsed < -1) return 0;
  if(!strcmp(name,"joyb_menu_activate") && parsed >= 31) return 0;
  /* joyb_speed >= 20 is the original autorun setting, not an array index.
   * Menu activation shifts a signed 32-bit 1; negative values disable it.
   * Comparison-only/unbound settings retain their full signed range.
   * Other valid numeric prefixes retain the original scanf behavior. */
  return snprintf(value,100,"%lld",parsed)>0;
}
static int config_scan(FILE *stream,const char *format,va_list args) {
  if (strcmp(format,"%79s %99[^\n]\n")) return EOF;
  char line[256]; unsigned used=0; int invalid=0, c;
  while ((c=__wrap_fgetc(stream))!=EOF && c!='\n') {
    if (!c) invalid=1;
    if (used<sizeof(line)-2) line[used++]=(char)c;
    else invalid=1;
  }
  if (!used && !invalid && c==EOF) return EOF;
  if (invalid) return 0;
  line[used]=0;
  char *name=va_arg(args,char *), *value=va_arg(args,char *);
  int fields=sscanf(line,"%79s %99[^\n]",name,value);
  if(fields==2 && !numeric_config_value(name,value)) return 0;
  return fields;
}
int __wrap_fscanf(FILE *stream,const char *format,...) {
  va_list args; va_start(args,format); int result=config_scan(stream,format,args); va_end(args); return result;
}
int __wrap___isoc99_fscanf(FILE *stream,const char *format,...) {
  va_list args; va_start(args,format); int result=config_scan(stream,format,args); va_end(args); return result;
}
int __wrap_remove(const char *path) {
  int k=kind(path);
  if (k>=3&&k<=8 && storage_error) { errno=EIO; return -1; }
  if (k>=3&&k<=8) { slot_lengths[k-3]=0; if(last_slot==k-3)last_slot=-1; dirty=1; return 0; }
  if (k==2) {temp_length=0; return 0;} return -1;
}
int __wrap_rename(const char *from,const char *to) {
  int k=kind(to); if(kind(from)!=2||k<3||k>8||!temp_length) return -1;
  memcpy(slots[k-3],temp_data,temp_length); slot_lengths[k-3]=temp_length;
  last_slot=k-3; dirty=1; temp_length=0; return 0;
}
int __wrap_mkdir(const char *path,mode_t mode) { (void)path;(void)mode; return -1; }
char *__wrap_getenv(const char *name) { (void)name; return NULL; }
int system(const char *command) { (void)command; return -1; }
int __wrap_access(const char *path,int mode) {
  int k=kind(path);
  if(k>=11) return (mode&2)?-1:0;
  return k==1 || (k>=3&&k<=8&&slot_lengths[k-3]) || (k>=9&&k<=10&&config_lengths[k-9])?0:-1;
}
