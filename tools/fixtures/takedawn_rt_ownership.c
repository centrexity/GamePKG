/* Compare unchanged ownership behavior against the pinned original source. */
#include <stdio.h>
#include "nv2a_backend_common.c"
#define CHECK(c) do {if(!(c)){fprintf(stderr,"CHECK_FAILED %s:%d\n",__FILE__,__LINE__);fflush(stderr);exit(1);}}while(0)
#include <pthread.h>
/* Generated directly from the pinned original ownership functions. */
#include "rt_hash_reference.inc"

static uint8_t *memory;
static void *worker(void *unused){(void)unused;struct nv2a_rt_own o={0};for(unsigned i=0;i<5000;i++){
 nv2a_rt_own_reset(&o,memory,0x1000,1024,8,i*2);CHECK(o.mem_hash==nv2a_tex_hash(memory+0x1000,8192,NV2A_TEX_HASH_SEED));
 CHECK(!nv2a_rt_own_stale(&o,memory,0x1000,1024,8,i*2+1));}return NULL;}
int main(void){
 memory=calloc(1,NV2A_RT_WINDOW);CHECK(memory);for(unsigned i=0;i<32u*1024u*1024u;i++)memory[i]=(uint8_t)(i*13u+(i>>12));
 struct nv2a_rt_own candidate[24]={0},original[24]={0};uint32_t flip=100;unsigned checked=0;
 for(unsigned i=0;i<8192;i++){
  unsigned k=(i*13u)%24;uint32_t addr=0x1000+k*1048576u,pitch=1024,height=(i%3)?256:512;
  if(i%11==0)addr=0x1000+(k/2)*1048576u; /* aliases */
  if(i%3==0)memset(memory+addr,0,pitch*height);
  nv2a_rt_own_reset(&candidate[k],memory,addr,pitch,height,flip);reference_rt_own_reset(&original[k],memory,addr,pitch,height,flip);
  if(i%5==0){nv2a_rt_own_drawn(&candidate[k],flip);nv2a_rt_own_drawn(&original[k],flip);}
  memory[addr+(i*31u)%(pitch*height)]^=1;
  for(unsigned phase=0;phase<3;phase++){
   int a=nv2a_rt_own_stale(&candidate[k],memory,addr,pitch,height,flip),b=reference_rt_own_stale(&original[k],memory,addr,pitch,height,flip);
   CHECK(a==b);CHECK(!memcmp(&candidate[k],&original[k],sizeof candidate[k]));CHECK(nv2a_rt_own_hashes()==reference_rt_own_hashes());
   checked++;flip++;
  }
 }
 memset(memory+0x1000,0,8192);
 pthread_t workers[4];uint64_t before=nv2a_rt_own_hashes();for(unsigned i=0;i<4;i++)CHECK(!pthread_create(&workers[i],NULL,worker,NULL));for(unsigned i=0;i<4;i++)CHECK(!pthread_join(workers[i],NULL));
 CHECK(nv2a_rt_own_hashes()==before+40000);
 free(memory);
 printf("PASS %u original ownership state/count comparisons, aliases, zero/sparse backing memory and 40000 threaded checks\n",checked);
}
