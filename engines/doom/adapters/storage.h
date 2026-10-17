/* GamePKG private DOOM files and versioned save bundle. GPL-2.0-or-later.
 * Exact virtual names only; no operation reaches a host file descriptor.
 */
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
  int used, writing, kind, error;
} File;
static File files[8];
static File *file(FILE *stream) {
  for (unsigned i=0;i<8;i++) if (stream==(FILE *)&files[i] && files[i].used) return &files[i];
  return NULL;
}
static int kind(const char *path) {
  if (!path) return 0;
  if (!strcmp(path,"doom.wad")) return 1;
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
  if (!k || (!writing&&!reading) || ((k==1||(k>=3&&k<=8))&&writing)) { errno=EACCES; return NULL; }
  uint32_t length=k==1?wad_length:k==2?temp_length:k<=8?slot_lengths[k-3]:config_lengths[k-9];
  if (!writing && !length) { errno=ENOENT; return NULL; }
  for (unsigned i=0;i<8;i++) if (files[i].used && files[i].kind==k && (files[i].writing||writing)) {
    errno=EBUSY; return NULL;
  }
  for (unsigned i=0;i<8;i++) if (!files[i].used) {
    File *f=&files[i];
    *f=(File){.used=1,.kind=k,.writing=writing,.size=writing?0:length};
    f->data=k==1?wad:k==2?temp_data:k<=8?slots[k-3]:writing?config_stage[k-9]:configs[k-9];
    f->capacity=k==1?wad_length:k<=8?SAVE_MAX:CONFIG_MAX;
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
  File *f=file(stream); if (!f||f->writing||!size||count>SIZE_MAX/size) return 0;
  size_t items=count; if (items>(f->size-f->pos)/size) items=(f->size-f->pos)/size;
  memcpy(out,f->data+f->pos,items*size); f->pos+=(uint32_t)(items*size); return items;
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
  f->pos=(uint32_t)(base+offset); return 0;
}
long __wrap_ftell(FILE *stream) { File *f=file(stream); return f?(long)f->pos:-1; }
int __wrap_feof(FILE *stream) { File *f=file(stream); return !f||f->pos==f->size; }
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
static int config_scan(FILE *stream,const char *format,va_list args) {
  if (strcmp(format,"%79s %99[^\n]\n")) return EOF;
  char line[256]; if (!__wrap_fgets(line,sizeof(line),stream)) return EOF;
  if (!strchr(line,'\n') && !__wrap_feof(stream)) { int c; do { c=__wrap_fgetc(stream); } while (c!=EOF&&c!='\n'); return 0; }
  char *name=va_arg(args,char *), *value=va_arg(args,char *);
  return sscanf(line,"%79s %99[^\n]",name,value);
}
int __wrap_fscanf(FILE *stream,const char *format,...) {
  va_list args; va_start(args,format); int result=config_scan(stream,format,args); va_end(args); return result;
}
int __wrap___isoc99_fscanf(FILE *stream,const char *format,...) {
  va_list args; va_start(args,format); int result=config_scan(stream,format,args); va_end(args); return result;
}
int __wrap_remove(const char *path) {
  int k=kind(path);
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
  (void)mode; int k=kind(path);
  return k==1 || (k>=3&&k<=8&&slot_lengths[k-3]) || (k>=9&&config_lengths[k-9])?0:-1;
}
