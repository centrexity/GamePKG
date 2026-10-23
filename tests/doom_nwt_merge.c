/* Exercise the unchanged pinned NWT implementation with metadata-only WADs. */
#include <assert.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <stdarg.h>
#include "w_wad.h"
#include "w_merge.h"
#include "m_misc.h"
static wad_file_t base_file, patch_file;
lumpinfo_t *lumpinfo;
unsigned int numlumps;
static const char *pending[8];
static unsigned pending_count, closed, opened;
static int missing;
wad_file_t *W_AddFile(char *name) {
    assert(!strcmp(name,"patch.wad"));opened++;
    if(missing) return NULL;
    lumpinfo_t *next=realloc(lumpinfo,(numlumps+pending_count)*sizeof(*lumpinfo));assert(next);lumpinfo=next;
    for(unsigned i=0;i<pending_count;i++) {
        lumpinfo_t *entry=&lumpinfo[numlumps+i];memset(entry,0,sizeof(*entry));
        memcpy(entry->name,pending[i],strlen(pending[i]));entry->wad_file=&patch_file;
        entry->position=1000+(int)i;entry->size=20+(int)i;
    }
    numlumps+=pending_count;return &patch_file;
}
void W_CloseFile(wad_file_t *file) {assert(file==&patch_file);closed++;}
boolean M_StringCopy(char *to,const char *from,size_t capacity) {
    assert(capacity==8 && !from[0]);to[0]=0;return true;
}
void I_Error(char *error,...) {(void)error;abort();}
void *Z_Malloc(int size,int tag,void *owner) {(void)tag;(void)owner;assert(size>0);void *p=malloc((size_t)size);assert(p);return p;}
void Z_Free(void *p) {free(p);}
#include "../engines/doom/merge/upstream/w_merge.c"
int main(void) {
    const char *names[]={"F_START","FLAT1","F_END","S_START","PISGA0","TROOA1A5","PISGA0","S_END","TITLEPIC"};
    for(unsigned shape=0;shape<4;shape++) {
        numlumps=sizeof(names)/sizeof(*names);lumpinfo=calloc(numlumps,sizeof(*lumpinfo));assert(lumpinfo);
        for(unsigned i=0;i<numlumps;i++) {
            memcpy(lumpinfo[i].name,names[i],strlen(names[i]));lumpinfo[i].wad_file=&base_file;
            lumpinfo[i].position=(int)i*100;lumpinfo[i].size=99+(int)i;
        }
        lumpinfo_t expected[9];memcpy(expected,lumpinfo,sizeof(expected));
        pending_count=0;pending[pending_count++]="SS_START";
        if(shape&1) pending[pending_count++]="pisga0";
        if(shape&2) pending[pending_count++]="trooa1a5";
        pending[pending_count++]="FLAT1";pending[pending_count++]="TITLEPIC";pending[pending_count++]="SS_END";
        if(shape&1) {expected[4].name[0]=0;expected[6].name[0]=0;}
        if(shape&2) expected[5].name[0]=0;
        opened=closed=0;missing=0;
        W_NWTDashMerge("patch.wad");
        assert(opened==1 && closed==1 && numlumps==9);
        assert(!memcmp(lumpinfo,expected,sizeof(expected)));
        W_NWTDashMerge("patch.wad");
        assert(opened==2 && closed==2 && numlumps==9);
        assert(!memcmp(lumpinfo,expected,sizeof(expected)));
        missing=1;W_NWTDashMerge("patch.wad");
        assert(opened==3 && closed==2 && numlumps==9);
        assert(!memcmp(lumpinfo,expected,sizeof(expected)));
        free(lumpinfo);lumpinfo=NULL;numlumps=0;
    }
}
