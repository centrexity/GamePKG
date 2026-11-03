/* Maintainer-only independent binary32 oracle. Players do not need MPFR.
 * Build with an MPFR development header and library, then capture stdout. */
#include <float.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <mpfr.h>

_Static_assert(sizeof(float)==4 && FLT_RADIX==2 && FLT_MANT_DIG==24 && FLT_MAX_EXP==128,
               "requires IEEE binary32");
static mpfr_t input, sine, cosine;
static int comma;
static uint32_t bits(float value) {
  uint32_t result;
  memcpy(&result,&value,sizeof(result));
  return result;
}
static void emit(uint32_t value) {
  float scalar;
  memcpy(&scalar,&value,sizeof(scalar));
  mpfr_set_flt(input,scalar,MPFR_RNDN);
  mpfr_sin(sine,input,MPFR_RNDN);
  mpfr_cos(cosine,input,MPFR_RNDN);
  printf("%s{\"bits\":%" PRIu32 ",\"sin\":%" PRIu32 ",\"cos\":%" PRIu32 "}",
         comma?",\n":"\n",value,bits(mpfr_get_flt(sine,MPFR_RNDN)),bits(mpfr_get_flt(cosine,MPFR_RNDN)));
  comma=1;
}
int main(void) {
  mpfr_init2(input,512);
  mpfr_init2(sine,512);
  mpfr_init2(cosine,512);
  printf("{\"oracle\":\"MPFR %s\",\"precision_bits\":512,\"rounding\":\"MPFR_RNDN\",\"cases\":[",mpfr_get_version());
  const uint32_t fractions[]={0,1,0x400000,0x7ffffe,0x7fffff};
  for(uint32_t exponent=0;exponent<255;exponent++)
    for(unsigned fraction=0;fraction<5;fraction++)
      for(uint32_t sign=0;sign<2;sign++)
        emit((sign<<31)|(exponent<<23)|fractions[fraction]);
  const uint32_t boundaries[]={0x3f490fda,0x3f490fdb,0x3f490fdc,0x3fc90fda,0x3fc90fdb,0x3fc90fdc,0x40490fda,0x40490fdb,0x40490fdc};
  for(unsigned i=0;i<sizeof(boundaries)/sizeof(boundaries[0]);i++) {
    emit(boundaries[i]);
    emit(boundaries[i]|0x80000000u);
  }
  const float rates[]={0.01f,0.02f,0.03f,0.05f,0.1f,0.15f,0.2f,0.3f};
  for(unsigned i=0;i<sizeof(rates)/sizeof(rates[0]);i++) {
    float angle=0x1p64f*rates[i];
    emit(bits(angle));
    emit(bits(-angle));
  }
  uint32_t seed=0x9e3779b9u;
  for(unsigned i=0;i<1024;i++) {
    seed=seed*1664525u+1013904223u;
    if((seed&0x7f800000u)!=0x7f800000u) emit(seed);
  }
  puts("\n]}");
  mpfr_clear(input);
  mpfr_clear(sine);
  mpfr_clear(cosine);
  return 0;
}
