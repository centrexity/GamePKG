#include <stdint.h>
#include <inttypes.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
uint64_t original_now(void),candidate_now(void);
int64_t clock_fixture_counter,clock_fixture_frequency;
static uint64_t rng=0xb6c817921173ab4full;
static uint64_t word(void){rng^=rng<<13;rng^=rng>>7;rng^=rng<<17;return rng;}
int main(void){
 static const int64_t frequencies[]={1,24,1000,1000000,3579545,10000000,24000000,100000000,1000000000,3000000000ll,4294967295ll,INT64_MAX};
 static const int64_t boundaries[]={INT64_MIN,INT64_MIN+1,-10000001,-10000000,-1,0,1,99,100,9999999,10000000,10000001,184467440737095515ll,184467440737095516ll,184467440737095517ll,184467440737095518ll,INT64_MAX-1,INT64_MAX};
 uint64_t checks=0,fast=0;
 for(unsigned f=0;f<sizeof frequencies/sizeof *frequencies;f++){
  clock_fixture_frequency=frequencies[f];
  for(unsigned n=0;n<1048576+sizeof boundaries/sizeof *boundaries;n++){
   if(n<sizeof boundaries/sizeof *boundaries)clock_fixture_counter=boundaries[n];
   else {uint64_t bits=word();clock_fixture_counter=(int64_t)bits;}
   uint64_t a=original_now(),b=candidate_now();
   if(a!=b){fprintf(stderr,"mismatch frequency=%"PRId64" counter=%"PRId64" original=%"PRIu64" candidate=%"PRIu64"\n",clock_fixture_frequency,clock_fixture_counter,a,b);return 1;}
   checks++;fast+=clock_fixture_frequency==10000000 && clock_fixture_counter>=0;
  }
 }
 printf("PASS exact_clock_cases=%"PRIu64" fast_path_cases=%"PRIu64" frequencies=12 negative_and_wrap_boundaries=true\n",checks,fast);return 0;
}
