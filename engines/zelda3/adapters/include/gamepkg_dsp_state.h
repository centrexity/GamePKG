/* GamePKG portable original 3024-byte DSP component. Apache-2.0.
 * Wire offsets describe the original format, independently of native layout. */
#pragma once
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
enum { GAMEPKG_DSP_STATE_SIZE=3024 };
enum { GAMEPKG_DSP_BYTES, GAMEPKG_DSP_WORDS, GAMEPKG_DSP_BOOLS };
typedef struct { size_t native,wire,count; unsigned kind; } GamepkgDspField;
#define GP_DSP_C(name,wire,kind,count) {offsetof(DspChannel,name),wire,count,GAMEPKG_DSP_##kind}
static const GamepkgDspField gamepkg_dsp_channel_fields[]={
  GP_DSP_C(pitch,0,WORDS,1),GP_DSP_C(pitchCounter,2,WORDS,1),GP_DSP_C(pitchModulation,4,BOOLS,1),
  GP_DSP_C(decodeBuffer,6,WORDS,19),GP_DSP_C(srcn,44,BYTES,1),GP_DSP_C(decodeOffset,46,WORDS,1),
  GP_DSP_C(previousFlags,48,BYTES,1),GP_DSP_C(old,50,WORDS,1),GP_DSP_C(older,52,WORDS,1),
  GP_DSP_C(useNoise,54,BOOLS,1),GP_DSP_C(adsrRates,56,WORDS,4),GP_DSP_C(rateCounter,64,WORDS,1),
  GP_DSP_C(adsrState,66,BYTES,1),GP_DSP_C(sustainLevel,68,WORDS,1),GP_DSP_C(useGain,70,BOOLS,1),
  GP_DSP_C(gainMode,71,BYTES,1),GP_DSP_C(directGain,72,BOOLS,1),GP_DSP_C(gainValue,74,WORDS,1),
  GP_DSP_C(gain,76,WORDS,1),GP_DSP_C(keyOn,78,BOOLS,1),GP_DSP_C(keyOff,79,BOOLS,1),
  GP_DSP_C(sampleOut,80,WORDS,1),GP_DSP_C(volumeL,82,BYTES,1),GP_DSP_C(volumeR,83,BYTES,1),
  GP_DSP_C(echoEnable,84,BOOLS,1)
};
#undef GP_DSP_C
#define GP_DSP_F(name,wire,kind,count) {offsetof(Dsp,name),wire,count,GAMEPKG_DSP_##kind}
static const GamepkgDspField gamepkg_dsp_fields[]={
  GP_DSP_F(ram,0,BYTES,128),GP_DSP_F(dirPage,816,WORDS,1),GP_DSP_F(evenCycle,818,BOOLS,1),
  GP_DSP_F(mute,819,BOOLS,1),GP_DSP_F(reset,820,BOOLS,1),GP_DSP_F(masterVolumeL,821,BYTES,1),
  GP_DSP_F(masterVolumeR,822,BYTES,1),GP_DSP_F(noiseSample,824,WORDS,1),GP_DSP_F(noiseRate,826,WORDS,1),
  GP_DSP_F(noiseCounter,828,WORDS,1),GP_DSP_F(echoWrites,830,BOOLS,1),GP_DSP_F(echoVolumeL,831,BYTES,1),
  GP_DSP_F(echoVolumeR,832,BYTES,1),GP_DSP_F(feedbackVolume,833,BYTES,1),GP_DSP_F(echoBufferAdr,834,WORDS,1),
  GP_DSP_F(echoDelay,836,WORDS,1),GP_DSP_F(echoRemain,838,WORDS,1),GP_DSP_F(echoBufferIndex,840,WORDS,1),
  GP_DSP_F(firBufferIndex,842,BYTES,1),GP_DSP_F(firValues,843,BYTES,8),GP_DSP_F(firBufferL,852,WORDS,8),
  GP_DSP_F(firBufferR,868,WORDS,8),GP_DSP_F(sampleBuffer,884,WORDS,1068),GP_DSP_F(sampleOffset,3020,WORDS,1)
};
#undef GP_DSP_F
static inline void gamepkg_dsp_encode_fields(const void *object,uint8_t *out,const GamepkgDspField *fields,size_t count) {
  const uint8_t *native=object;
  for(size_t f=0;f<count;f++) {
    const GamepkgDspField *field=&fields[f];
    for(size_t i=0;i<field->count;i++) {
      if(field->kind==GAMEPKG_DSP_WORDS) {
        uint16_t value;memcpy(&value,native+field->native+i*2,2);
        out[field->wire+i*2]=(uint8_t)value;out[field->wire+i*2+1]=(uint8_t)(value>>8);
      } else if(field->kind==GAMEPKG_DSP_BOOLS) {
        bool value;memcpy(&value,native+field->native+i*sizeof(bool),sizeof(value));
        out[field->wire+i]=value;
      } else out[field->wire+i]=native[field->native+i];
    }
  }
}
static inline int gamepkg_dsp_fields_valid(const uint8_t *in,const GamepkgDspField *fields,size_t count) {
  for(size_t f=0;f<count;f++)if(fields[f].kind==GAMEPKG_DSP_BOOLS)
    for(size_t i=0;i<fields[f].count;i++)if(in[fields[f].wire+i]>1)return 0;
  return 1;
}
static inline void gamepkg_dsp_decode_fields(void *object,const uint8_t *in,const GamepkgDspField *fields,size_t count) {
  uint8_t *native=object;
  for(size_t f=0;f<count;f++) {
    const GamepkgDspField *field=&fields[f];
    for(size_t i=0;i<field->count;i++) {
      if(field->kind==GAMEPKG_DSP_WORDS) {
        uint16_t value=(uint16_t)(in[field->wire+i*2]|(uint16_t)in[field->wire+i*2+1]<<8);
        memcpy(native+field->native+i*2,&value,2);
      } else if(field->kind==GAMEPKG_DSP_BOOLS) {
        bool value=in[field->wire+i]!=0;
        memcpy(native+field->native+i*sizeof(bool),&value,sizeof(value));
      } else native[field->native+i]=in[field->wire+i];
    }
  }
}
static inline void gamepkg_dsp_encode(const Dsp *dsp,uint8_t out[GAMEPKG_DSP_STATE_SIZE]) {
  memset(out,0,GAMEPKG_DSP_STATE_SIZE);
  gamepkg_dsp_encode_fields(dsp,out,gamepkg_dsp_fields,sizeof(gamepkg_dsp_fields)/sizeof(*gamepkg_dsp_fields));
  for(unsigned i=0;i<8;i++)gamepkg_dsp_encode_fields(&dsp->channel[i],out+128+i*86,
    gamepkg_dsp_channel_fields,sizeof(gamepkg_dsp_channel_fields)/sizeof(*gamepkg_dsp_channel_fields));
}
static inline int gamepkg_dsp_valid(const uint8_t in[GAMEPKG_DSP_STATE_SIZE]) {
  if(!gamepkg_dsp_fields_valid(in,gamepkg_dsp_fields,sizeof(gamepkg_dsp_fields)/sizeof(*gamepkg_dsp_fields)) ||
      in[842]>7 || (in[3020]|(unsigned)in[3021]<<8)>534)return 0;
  for(unsigned i=0;i<8;i++) {
    const uint8_t *p=in+128+i*86;
    if(!gamepkg_dsp_fields_valid(p,gamepkg_dsp_channel_fields,
        sizeof(gamepkg_dsp_channel_fields)/sizeof(*gamepkg_dsp_channel_fields)) ||
        p[48]>3 || p[66]>4 || p[71]>3)return 0;
  }
  return 1;
}
static inline int gamepkg_dsp_decode(Dsp *dsp,const uint8_t in[GAMEPKG_DSP_STATE_SIZE]) {
  if(!gamepkg_dsp_valid(in))return 0;
  Dsp staged=*dsp;
  gamepkg_dsp_decode_fields(&staged,in,gamepkg_dsp_fields,sizeof(gamepkg_dsp_fields)/sizeof(*gamepkg_dsp_fields));
  for(unsigned i=0;i<8;i++)gamepkg_dsp_decode_fields(&staged.channel[i],in+128+i*86,
    gamepkg_dsp_channel_fields,sizeof(gamepkg_dsp_channel_fields)/sizeof(*gamepkg_dsp_channel_fields));
  *dsp=staged;return 1;
}
static inline void gamepkg_dsp_saveload(Dsp *dsp,SaveLoadFunc *func,void *ctx) {
  uint8_t bytes[GAMEPKG_DSP_STATE_SIZE];gamepkg_dsp_encode(dsp,bytes);
  func(ctx,bytes,sizeof(bytes));if(!gamepkg_dsp_decode(dsp,bytes))abort();
}
