/* Asset-free differential fixture; original hash is supplied separately. */
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#define CHECK(c) do {if(!(c)){fprintf(stderr,"CHECK_FAILED %s:%d\n",__FILE__,__LINE__);fflush(stderr);exit(1);}}while(0)
#include <inttypes.h>
#if defined(__SSE2__)
#include <immintrin.h>
#define FP_MODES 16
static unsigned read_csr(void){return _mm_getcsr();}
static void write_csr(unsigned value){_mm_setcsr(value);}
#else
#define FP_MODES 1
static unsigned read_csr(void){return 0;}
static void write_csr(unsigned value){(void)value;}
#endif
#ifdef _WIN32
#include <windows.h>
#else
#include <time.h>
#include <sys/mman.h>
#include <unistd.h>
#endif
#include "rt_zero_hash.h"
static uint64_t rng=0x851c38e5;static volatile uint64_t sink;
static uint64_t next(void){rng^=rng<<13;rng^=rng>>7;rng^=rng<<17;return rng;}
static uint64_t ns(void){
#ifdef _WIN32
 LARGE_INTEGER t,f;QueryPerformanceCounter(&t);QueryPerformanceFrequency(&f);return (uint64_t)(t.QuadPart/f.QuadPart)*1000000000ull+(uint64_t)(t.QuadPart%f.QuadPart)*1000000000ull/f.QuadPart;
#else
 struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (uint64_t)t.tv_sec*1000000000ull+t.tv_nsec;
#endif
}
int main(int argc,char **argv){
#ifdef _WIN32
 SetErrorMode(SEM_FAILCRITICALERRORS|SEM_NOGPFAULTERRORBOX);
#endif
 uint8_t *m=calloc(1,1048640);CHECK(m);unsigned cases=0,saved=read_csr();
 for(unsigned mode=0;mode<FP_MODES;mode++)for(unsigned align=0;align<64;align++)for(unsigned i=0;i<160;i++){
  uint32_t n=i<128?i:((i-128)*257+255);uint64_t seed=next();
  unsigned csr=0x1f80|((mode&3)<<13)|((mode&4)?0x8000:0)|((mode&8)?0x40:0);write_csr(csr);
  uint64_t a=nv2a_tex_hash(m+align,n,seed);unsigned fa=read_csr();uint64_t b=gamepkg_rt_hash(m+align,n,seed);unsigned fb=read_csr();CHECK(a==b&&fa==fb);cases++;
 }
 write_csr(saved);
 for(unsigned i=0;i<65536;i++){
  unsigned byte=(unsigned)(next()%8192),bit=(unsigned)(next()%8);uint64_t seed=next();m[byte]=(uint8_t)(1u<<bit);
  CHECK(gamepkg_rt_hash(m,8192,seed)==nv2a_tex_hash(m,8192,seed));m[byte]=0;cases++;
 }
 for(unsigned i=0;i<32768;i++){
  unsigned n=(unsigned)(next()%8193),prefix=(unsigned)(next()%(n+1)),offset=(unsigned)(next()%64);uint64_t seed=next();
  memset(m,0,8256);for(unsigned k=prefix;k<n;k++)m[offset+k]=(uint8_t)next();
  CHECK(gamepkg_rt_hash(m+offset,n,seed)==nv2a_tex_hash(m+offset,n,seed));cases++;
 }
 memset(m,0,1048640);
 size_t page;uint8_t *guard;
#ifdef _WIN32
 SYSTEM_INFO si;GetSystemInfo(&si);page=si.dwPageSize;guard=VirtualAlloc(NULL,page*2,MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);DWORD old;CHECK(guard&&VirtualProtect(guard+page,page,PAGE_NOACCESS,&old));
#else
 page=(size_t)sysconf(_SC_PAGESIZE);guard=mmap(NULL,page*2,PROT_READ|PROT_WRITE,MAP_PRIVATE|MAP_ANONYMOUS,-1,0);CHECK(guard!=MAP_FAILED&&!mprotect(guard+page,page,PROT_NONE));
#endif
 memset(guard,0,page);
 for(uint32_t n=0;n<=page;n++){
  uint64_t seed=next();uint8_t *p=guard+page-n;CHECK(gamepkg_rt_hash(p,n,seed)==nv2a_tex_hash(p,n,seed));
  if(n){p[n-1]=0x80;CHECK(gamepkg_rt_hash(p,n,seed)==nv2a_tex_hash(p,n,seed));p[n-1]=0;}cases++;
 }
 CHECK(gamepkg_rt_hash(NULL,0,77)==nv2a_tex_hash(NULL,0,77));
#ifdef _WIN32
 VirtualFree(guard,0,MEM_RELEASE);
#else
 munmap(guard,page*2);
#endif
 printf("PASS cases=%u pointer_bits=%u alignments64 FP_modes=%u exact_zero_prefix sparse_mutations protected_boundaries\n",cases,(unsigned)(sizeof(void *)*8),FP_MODES);
 if(argc>1&&!strcmp(argv[1],"--benchmark")){
  uint32_t sizes[]={256,4096,65536,1048576};
  for(unsigned k=0;k<4;k++)for(unsigned kind=0;kind<3;kind++)for(unsigned r=0;r<8;r++)for(unsigned phase=0;phase<2;phase++){
   uint32_t n=sizes[k];memset(m,0,n);if(kind==1)m[0]=1;if(kind==2)m[n-1]=1;
   unsigned candidate=(r+phase)%2,iterations=20000000u/n+20;uint64_t v=next(),t=ns();
   for(unsigned j=0;j<iterations;j++)v^=candidate?gamepkg_rt_hash(m,n,v):nv2a_tex_hash(m,n,v);
   uint64_t elapsed=ns()-t;sink=v;printf("BENCH bytes=%u kind=%u round=%u candidate=%u ns=%" PRIu64 " iterations=%u\n",n,kind,r,candidate,elapsed,iterations);
  }
 }
 free(m);return 0;
}
