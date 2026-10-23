/* Original DSP callback: stable bytes, live-pointer isolation and stereo state. */
#include <assert.h>
#include <stdio.h>
#include <string.h>
#ifndef GAMEPKG_DSP_SOURCE
#define GAMEPKG_DSP_SOURCE "../engines/zelda3/upstream/snes/dsp.c"
#endif
#include GAMEPKG_DSP_SOURCE
#include "../engines/zelda3/adapters/include/gamepkg_dsp_state.h"
#ifdef GAMEPKG_TEST_32
_Static_assert(sizeof(void*)==4,"actual 32-bit process required");
#endif
static uint8_t ram[65536],other_ram[65536],saved[3024];
static void save(void *ctx,void *data,size_t size) {assert(ctx==saved && size==3024);memcpy(saved,data,size);}
static void load(void *ctx,void *data,size_t size) {assert(ctx==saved && size==3024);memcpy(data,saved,size);}
static Dsp specimen(void) {
  Dsp dsp;memset(&dsp,0xa5,sizeof(dsp));dsp.apu_ram=ram;
  for(unsigned i=0;i<8;i++) {
    DspChannel *c=&dsp.channel[i];c->pitch=0x1234+i;c->pitchCounter=0x5678+i;
    c->pitchModulation=true;c->useNoise=false;c->useGain=true;c->directGain=false;
    c->keyOn=true;c->keyOff=false;c->echoEnable=true;
    c->adsrState=i%5;c->gainMode=i%4;c->previousFlags=i%4;
    c->decodeBuffer[0]=INT16_MIN;c->decodeBuffer[1]=-1;c->decodeBuffer[18]=INT16_MAX;
    c->sampleOut=-1234;c->volumeL=INT8_MIN;c->volumeR=INT8_MAX;
  }
  dsp.evenCycle=true;dsp.mute=false;dsp.reset=true;dsp.echoWrites=false;
  dsp.masterVolumeL=-127;dsp.masterVolumeR=126;dsp.firBufferIndex=7;dsp.sampleOffset=534;
  for(unsigned i=0;i<1068;i++)dsp.sampleBuffer[i]=(int16_t)((int)(i*37)-32768);
  return dsp;
}
static int padding(unsigned byte) {
  if(byte>=128 && byte<816) {
    unsigned p=(byte-128)%86;
    return p==5 || p==45 || p==49 || p==55 || p==67 || p==73 || p==85;
  }
  return byte==823 || byte==851 || byte>=3022;
}
static unsigned maximum(unsigned byte) {
  if(byte>=128 && byte<816) {
    unsigned p=(byte-128)%86;
    if(p==4 || p==54 || p==70 || p==72 || p==78 || p==79 || p==84)return 1;
    if(p==48 || p==71)return 3;
    if(p==66)return 4;
  }
  if(byte==818 || byte==819 || byte==820 || byte==830)return 1;
  return byte==842?7:255;
}
static void reject(const uint8_t *bytes) {
  Dsp candidate=specimen();uint8_t before[sizeof(candidate)];memcpy(before,&candidate,sizeof(candidate));
  assert(!gamepkg_dsp_decode(&candidate,bytes));assert(!memcmp(before,&candidate,sizeof(candidate)));
}
int main(void) {
  Dsp dsp=specimen();dsp_saveload(&dsp,save,saved);
  for(unsigned i=0;i<3024;i++)if(padding(i))assert(saved[i]==0);
  assert(saved[128]==0x34 && saved[129]==0x12 && saved[130]==0x78 && saved[131]==0x56);
  assert(saved[134]==0 && saved[135]==0x80 && saved[136]==0xff && saved[137]==0xff);
  assert(saved[208]==0x2e && saved[209]==0xfb && saved[210]==0x80 && saved[211]==0x7f);
  /* On the original little-endian layouts all semantic bytes match, while
   * padding is canonical. The codec itself never assumes those native offsets. */
  uint16_t endian=1;
  if(*(uint8_t *)&endian==1 && sizeof(Dsp)-offsetof(Dsp,ram)==3024) {
    const uint8_t *raw=(const uint8_t *)&dsp.ram;
    for(unsigned i=0;i<3024;i++)if(!padding(i))assert(saved[i]==raw[i]);
  }
  uint8_t golden[3024];memcpy(golden,saved,sizeof(golden));
  Dsp loaded={.apu_ram=other_ram};dsp_saveload(&loaded,load,saved);
  assert(loaded.apu_ram==other_ram);
  dsp_saveload(&loaded,save,saved);assert(!memcmp(saved,golden,sizeof(golden)));
  int16_t before[1600],after[1600];dsp_getSamples(&dsp,before,800,2);dsp_getSamples(&loaded,after,800,2);
  assert(!memcmp(before,after,sizeof(before)) && before[0]!=before[1]);
  unsigned rejected=0;
  for(unsigned byte=0;byte<3024;byte++) {
    unsigned max=maximum(byte);
    if(max==255)continue;
    for(unsigned value=0;value<256;value++) {
      uint8_t changed[3024];memcpy(changed,golden,sizeof(changed));changed[byte]=(uint8_t)value;
      if(value>max){reject(changed);rejected++;}
      else {
        Dsp candidate=specimen();uint8_t encoded[3024];assert(gamepkg_dsp_decode(&candidate,changed));
        gamepkg_dsp_encode(&candidate,encoded);assert(!memcmp(encoded,changed,sizeof(encoded)));
      }
    }
  }
  for(unsigned value=0;value<=65535;value++) {
    uint8_t changed[3024];memcpy(changed,golden,sizeof(changed));changed[3020]=(uint8_t)value;changed[3021]=(uint8_t)(value>>8);
    if(value>534){reject(changed);rejected++;}
    else {Dsp candidate=specimen();assert(gamepkg_dsp_decode(&candidate,changed));assert(candidate.sampleOffset==value);}
  }
  memcpy(saved,golden,sizeof(saved));
  for(unsigned i=0;i<3024;i++)if(padding(i))saved[i]=0x5a;
  dsp_saveload(&loaded,load,saved);dsp_saveload(&loaded,save,saved);
  assert(!memcmp(saved,golden,sizeof(saved)));
  printf("DSP state: 3024 canonical bytes, exact original stereo sample-buffer output, %u invalid fields rejected without mutation; pointer width %zu.\n",rejected,sizeof(void*));
  return 0;
}
