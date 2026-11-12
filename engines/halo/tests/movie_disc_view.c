/* SPDX-License-Identifier: Apache-2.0 */

#include <assert.h>
#include <stdint.h>
#include <string.h>
#include <pthread.h>
#include "xiso.h"
void platform_log(const char *format, ...) {(void)format;}
static int movie;
static void *reader(void *argument) {
 uintptr_t seed=(uintptr_t)argument;
 for(unsigned i=0;i<1000;++i){uint8_t byte=255;unsigned at=(unsigned)(i*7+seed*11)%32;
  assert(xiso_view_read(movie,at,&byte,1)==1&&byte==at);}
 return 0;
}
int main(int argc,char **argv){
 assert(argc==3);char error[1024];int okay=xiso_view_mount(argv[1],"/private-test-root",error,sizeof(error));
 if(!strcmp(argv[2],"bad")){assert(!okay);return 0;}assert(okay);
 movie=xiso_view_lookup("/private-test-root/bink/intro.bik");assert(movie>=0&&xiso_view_size(movie)==32);
 assert(xiso_view_lookup("/PRIVATE-TEST-ROOT/BINK/INTRO.BIK")==movie);
 const char *bad[]={"/private-test-root/bink/../maps/ui.map","/private-test-root/bink/../../outside",
  "/private-test-root/bink/intro.bik:stream","/private-test-root/bink/./intro.bik","/private-test-root/maps/intro.bik",
  "/private-test-root/bink/ui.map","/private-test-root/bink/intro.bik/child"};
 for(unsigned i=0;i<sizeof(bad)/sizeof(bad[0]);++i)assert(xiso_view_lookup(bad[i])==-1);
 gamepkg_path_info info;assert(xiso_view_stat("/private-test-root/bink/intro.bik",&info)==1);
 assert(info.kind==GAMEPKG_PATH_FILE&&info.size==32&&(info.flags&GAMEPKG_PATH_READ_ONLY));
 uint8_t bytes[64];assert(xiso_view_read(movie,0,bytes,sizeof(bytes))==32);
 for(unsigned i=0;i<32;++i)assert(bytes[i]==i);
 assert(xiso_view_read(movie,32,bytes,1)==0&&xiso_view_read(movie,UINT64_MAX,bytes,1)==0);
 assert(xiso_view_read(movie,0,0,1)==-1);
 uint32_t directory;assert(xiso_view_dir_open("/private-test-root",&directory)==1);
 assert(xiso_view_dir_read(directory,bytes,sizeof(bytes))==4&&!strcmp((char*)bytes,"maps"));
 assert(xiso_view_dir_read(directory,bytes,sizeof(bytes))==4&&!strcmp((char*)bytes,"bink"));
 assert(!xiso_view_dir_read(directory,bytes,sizeof(bytes)));assert(xiso_view_dir_close(directory));
 assert(xiso_view_dir_open("/private-test-root/maps",&directory)==1);
 assert(xiso_view_dir_read(directory,bytes,sizeof(bytes))==6&&!strcmp((char*)bytes,"ui.map"));
 assert(!xiso_view_dir_read(directory,bytes,sizeof(bytes)));assert(xiso_view_dir_close(directory));
 assert(xiso_view_dir_open("/private-test-root/bink",&directory)==1);
 assert(xiso_view_dir_read(directory,bytes,sizeof(bytes))==9&&!strcmp((char*)bytes,"intro.bik"));
 assert(!xiso_view_dir_read(directory,bytes,sizeof(bytes)));assert(xiso_view_dir_close(directory));
 pthread_t workers[8];for(uintptr_t i=0;i<8;++i)assert(!pthread_create(workers+i,0,reader,(void*)i));
 for(unsigned i=0;i<8;++i)assert(!pthread_join(workers[i],0));return 0;
}
