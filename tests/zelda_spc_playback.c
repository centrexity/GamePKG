// Complete production SPC player/DSP, synthetic banks only. No game media.
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "src/types.h"
#include GAMEPKG_SPC_PLAYER_SOURCE
#include "gamepkg_dsp_state.h"
#include GAMEPKG_SPC_HELPERS
void gamepkg_test_decode_brr(Dsp *dsp,int ch);
void gamepkg_legacy_brr(Dsp *dsp,int ch);
static uint8 ram[65536],expected[65536];
static union {uint16_t alignment;uint8 bytes[65535+4+2+7];} bank;
static uint64_t digest=UINT64_C(14695981039346656037);
static void hash(uint8 value) {digest^=value;digest*=UINT64_C(1099511628211);}
static void put16(uint8 *p,unsigned value) {p[0]=(uint8)value;p[1]=(uint8)(value>>8);}
static int floor_div(int value,int denominator) {return value>=0?value/denominator:-((-value+denominator-1)/denominator);}
static void bank_validation(void) {
  assert(!gamepkg_spc_bank_valid(NULL,0));
  for(unsigned length=1;length<=65535;length++) {
    uint8 *allocation=calloc(1,length+7);assert(allocation);
    uint8 *data=allocation+1;
    put16(data,length);put16(data+2,65535);
    assert(gamepkg_spc_bank_valid(data,length+6));
    assert(!gamepkg_spc_bank_valid(data,length+5));
    assert(!gamepkg_spc_bank_valid(data,length+4));
    if(length==1 || length==2 || length==255 || length==256 || length==257 || length==65535)
      for(unsigned cut=0;cut<length+6;cut++)assert(!gamepkg_spc_bank_valid(data,cut));
    data[length+4]=1;assert(!gamepkg_spc_bank_valid(data,length+6));
    free(allocation);
  }
  uint8 terminal[4]={0};
  assert(gamepkg_spc_bank_valid(terminal,2));
  for(unsigned length=0;length<=4;length++)if(length!=2)assert(!gamepkg_spc_bank_valid(terminal,length));
  const uint8 multiple[]={1,0,255,255,7,2,0,0,0,8,9,0,0};
  assert(gamepkg_spc_bank_valid(multiple,sizeof(multiple)));
  for(unsigned cut=0;cut<sizeof(multiple);cut++)assert(!gamepkg_spc_bank_valid(multiple,cut));
}
static unsigned brr_decoder(SpcPlayer *p) {
  const int history[]={-32768,-16384,-1,0,1,16383,32767,8191};
  const unsigned addresses[]={1,0x4000,65535};unsigned cases=0;
  for(unsigned channel=0;channel<8;channel++)for(unsigned range=0;range<16;range++)for(unsigned filter=0;filter<4;filter++)for(unsigned flags=0;flags<4;flags++)for(unsigned nibble=0;nibble<16;nibble++)for(unsigned h=0;h<8;h++)for(unsigned location=0;location<3;location++) {
    DspChannel *c=&p->dsp->channel[channel];memset(c,0,sizeof(*c));
    unsigned address=addresses[location];c->decodeOffset=(uint16)address;c->old=(int16)history[h];c->older=(int16)history[(h+3)%8];
    p->ram[address]=(uint8)((range<<4)|(filter<<2)|flags);
    for(unsigned byte=1;byte<=8;byte++)p->ram[(address+byte)&65535u]=(uint8)(nibble*17u);
    int old=c->old,older=c->older;gamepkg_test_decode_brr(p->dsp,(int)channel);
    for(unsigned sample=0;sample<16;sample++) {
      int value=nibble<8?(int)nibble:(int)nibble-16;
      value=range<=12?floor_div(value*(1<<(int)range),2):(value<0?-4096:0);
      switch(filter) {
        case 1:value+=old+floor_div(-old,16);break;
        case 2:value+=2*old+floor_div(-3*old,32)-older+floor_div(older,16);break;
        case 3:value+=2*old+floor_div(-13*old,64)-older+floor_div(3*older,16);break;
      }
      if(value<-32768) {value=-32768;}
      if(value>32767) {value=32767;}
      int clipped=(int)((uint32_t)value&32767u);if(clipped>=16384)clipped-=32768;
      assert(c->decodeBuffer[sample+3]==clipped);older=old;old=clipped;
    }
    assert(c->old==old && c->older==older && c->decodeOffset==(uint16)(address+9) && c->previousFlags==flags);
    cases++;
  }
  return cases;
}
static void dispose(SpcPlayer *p) {dsp_free(p->dsp);free(p);}
static void primitive_access(void) {
  uint8 bytes[10];
  for(unsigned value=0;value<65536;value++) {
    for(unsigned offset=0;offset<8;offset++) {
      put16(bytes+offset,value);assert(gamepkg_spc_read_le16(bytes+offset)==value);
    }
    assert(gamepkg_spc_high((uint16)value)==value/256);
    for(unsigned high=0;high<256;high+=17)assert(gamepkg_spc_with_high((uint16)value,(uint8)high)==(value%256+high*256));
  }
  for(unsigned i=0;i<65536;i++)ram[i]=(uint8)(i*37u+(i>>8)*11u);
  for(unsigned address=0;address<65536;address++) {
    unsigned value=ram[address]+256u*ram[(address+1)&65535u];
    assert(gamepkg_spc_read_ram(ram,address)==value);
    assert(gamepkg_spc_read_ram(ram,address+65536u)==value);
  }
  assert(gamepkg_spc_read_ram(ram,UINT32_MAX)==ram[65535]+256u*ram[0]);
}
static unsigned live_high_bytes(SpcPlayer *a,SpcPlayer *b,int little) {
  unsigned mismatches=0;
  assert(a->tempo==0x1000 && b->tempo==0x1000);
  for(unsigned seed=0;seed<65536;seed++) {
    a->master_volume=b->master_volume=(uint16)(seed^0x1144);a->tempo=b->tempo=(uint16)seed;
    for(unsigned i=0;i<8;i++)a->channel[i].channel_volume=b->channel[i].channel_volume=(uint16)(seed+i*257u);
    Music_ResetChan(a);reference_reset(b);
    assert(a->master_volume==(0xc000u|((seed^0x1144)&255u)) && a->tempo==(0x2000u|(seed&255u)));
    for(unsigned i=0;i<8;i++) {
      assert(a->channel[i].channel_volume==(0xff00u|((seed+i*257u)&255u)));
      if(little)assert(a->channel[i].channel_volume==b->channel[i].channel_volume);
      else mismatches+=a->channel[i].channel_volume!=b->channel[i].channel_volume;
    }
    a->master_volume=b->master_volume=(uint16)seed;a->byte_3E1=b->byte_3E1=0;
    a->port_to_snes[0]=b->port_to_snes[0]=0;a->new_value_from_snes[0]=b->new_value_from_snes[0]=0xf2;
    Port0_HandleMusic(a);reference_music(b);
    assert(a->byte_3E1==seed/256 && a->master_volume==(0x7000u|(seed&255u)));
    if(little)assert(a->master_volume==b->master_volume && a->byte_3E1==b->byte_3E1);
    else mismatches+=a->master_volume!=b->master_volume || a->byte_3E1!=b->byte_3E1;
    a->new_value_from_snes[0]=b->new_value_from_snes[0]=0xf3;
    Port0_HandleMusic(a);reference_music(b);
    assert(a->master_volume==(seed/256?seed:(0x7000u|(seed&255u))) && a->byte_3E1==0);
    a->sfx_timer_accum=b->sfx_timer_accum=0;a->port_to_snes[0]=b->port_to_snes[0]=0;
    a->new_value_from_snes[0]=b->new_value_from_snes[0]=0;a->tempo=b->tempo=(uint16)seed;
    a->main_tempo_accum=b->main_tempo_accum=(uint8)(seed*17u);
    Spc_Loop_Part2(a,1);reference_tick(b,1);
    assert(a->main_tempo_accum==((seed*17u+seed/256)&255u));
    if(little)assert(a->main_tempo_accum==b->main_tempo_accum);
    else mismatches+=a->main_tempo_accum!=b->main_tempo_accum;
  }
  return mismatches;
}
static unsigned uploads(SpcPlayer *p,SpcPlayer *reference,int little) {
  const unsigned lengths[]={1,2,3,255,256,257,511,512,1024,65535};
  const unsigned targets[]={0,1,0x1234,65534,65535};unsigned cases=0;
  for(unsigned alignment=0;alignment<8;alignment++)for(unsigned size=0;size<sizeof(lengths)/sizeof(*lengths);size++)for(unsigned target=0;target<5;target++) {
    unsigned length=lengths[size],address=targets[target];uint8 *data=bank.bytes+alignment;
    memset(p->ram,0x5a,65536);memset(expected,0x5a,65536);
    put16(data,length);put16(data+2,address);
    for(unsigned i=0;i<length;i++)data[4+i]=expected[(address+i)&65535u]=(uint8)(i*37u+11u);
    put16(data+4+length,0);
    p->pause_music_ctr=19;p->port_to_snes[0]=7;p->port1_active=p->port2_active=p->port3_active=p->is_chan_on=0xff;
    memset(p->input_ports,0xff,4);SpcPlayer_Upload(p,data);
    assert(memcmp(p->ram,expected,65536)==0);
    assert(p->pause_music_ctr==0 && p->port_to_snes[0]==0 && !p->port1_active && !p->port2_active && !p->port3_active && !p->is_chan_on);
    for(unsigned i=0;i<4;i++)assert(!p->input_ports[i]);
    cases++;
  }
  // Aligned palindromic length stays bounded in the original BE decoder; its
  // nonpalindromic destination still directly exposes the old word-order bug.
  uint8 *data=bank.bytes;put16(data,514);put16(data+2,0x1234);
  for(unsigned i=0;i<514;i++) {data[4+i]=(uint8)(i*13u+1u);}
  put16(data+518,0);
  memset(p->ram,0,65536);memset(reference->ram,0,65536);
  SpcPlayer_Upload(p,data);reference_upload(reference,data);
  if(little)assert(memcmp(p->ram,reference->ram,65536)==0);else assert(memcmp(p->ram,reference->ram,65536)!=0);
  return cases;
}
static void block(uint8 *data,unsigned *offset,unsigned target,const uint8 *bytes,unsigned length) {
  put16(data+*offset,length);put16(data+*offset+2,target);memcpy(data+*offset+4,bytes,length);*offset+=length+4;
}
static uint64_t synthetic_song(SpcPlayer *p,unsigned range,unsigned filter,int tremolo,unsigned delay,unsigned rate) {
  SpcPlayer_Initialize(p);
  uint8 *data=bank.bytes+1;unsigned offset=0;
  const uint8 root[]={1,0x20},order[]={1,0x21,0,0};
  const uint8 phrase[16]={1,0x22};
  const uint8 notes[]={0xe1,5,0x18,0x7f,0xa4,0x18,0x7f,0xa8,0x18,0x7f,0xac,0};
  const uint8 modulated[]={0xe1,5,0xeb,(uint8)delay,(uint8)rate,(uint8)(tremolo<0?0:tremolo),0x18,0x7f,0xa4,0xec,0x18,0x7f,0xa8,0x18,0x7f,0xac,0};
  const uint8 instrument[]={0,0,0,0x7f,0x10,0};
  const uint8 directory[]={0,0x40,0,0x40};
  const uint8 sample[]={ (uint8)((range<<4)|(filter<<2)|3),0x17,0x39,0x5b,0x7d,0x9f,0xb1,0xd3,0xf5};
  block(data,&offset,0xd000,root,sizeof(root));block(data,&offset,0x2001,order,sizeof(order));
  block(data,&offset,0x2101,phrase,sizeof(phrase));block(data,&offset,0x2201,tremolo<0?notes:modulated,tremolo<0?sizeof(notes):sizeof(modulated));
  block(data,&offset,0x3d00,instrument,sizeof(instrument));block(data,&offset,0x3c00,directory,sizeof(directory));block(data,&offset,0x4000,sample,sizeof(sample));put16(data+offset,0);
  SpcPlayer_Upload(p,data);p->input_ports[0]=1;
  unsigned audible=0,separated=0,enabled=0,disabled_after=0;int16 output[1600];
  uint64_t pcm=UINT64_C(14695981039346656037);
  for(unsigned frame=0;frame<120;frame++) {
    SpcPlayer_GenerateSamples(p);dsp_getSamples(p->dsp,output,800,2);
    if(p->channel[0].tremolo_depth)enabled++;else if(enabled)disabled_after++;
    for(unsigned i=0;i<1600;i++) {uint16 value=(uint16)output[i];hash((uint8)value);hash((uint8)(value>>8));pcm^=(uint8)value;pcm*=UINT64_C(1099511628211);pcm^=(uint8)(value>>8);pcm*=UINT64_C(1099511628211);audible+=output[i]!=0;}
    for(unsigned i=0;i<800;i++)separated+=output[i*2]!=output[i*2+1];
  }
  assert(audible>0 && separated>0);
  if(tremolo>0)assert(enabled>0 && disabled_after>0);
  uint8 encoded[GAMEPKG_DSP_STATE_SIZE];gamepkg_dsp_encode(p->dsp,encoded);
  for(unsigned i=0;i<sizeof(encoded);i++)hash(encoded[i]);
  SpcPlayer_CopyVariablesToRam(p);for(unsigned i=0;i<65536;i++)hash(p->ram[i]);
  return pcm;
}
int main(int argc,char **argv) {
  const uint16 probe=0x1234;int little=*(const uint8 *)&probe==0x34;
#ifdef GAMEPKG_EXPECT_BIG_ENDIAN
  assert(!little);
#endif
#ifdef GAMEPKG_TEST_32
  _Static_assert(sizeof(void *)==4,"not actual 32-bit playback");
#endif
  SpcPlayer *a=SpcPlayer_Create(),*b=SpcPlayer_Create();SpcPlayer_Initialize(a);SpcPlayer_Initialize(b);
  if(argc>1) {
    if(strcmp(argv[1],"--legacy-brr")==0) {
      a->dsp->channel[0].decodeOffset=0x4000;a->dsp->channel[0].previousFlags=0;
      a->ram[0x4000]=3;memset(a->ram+0x4001,0x98,8);
      puts("LEGACY_NEGATIVE_BRR");fflush(stdout);gamepkg_legacy_brr(a->dsp,0);dispose(a);dispose(b);return 0;
    }
    assert(strcmp(argv[1],"--legacy-unaligned-upload")==0);
    uint8 *data=bank.bytes+1;put16(data,257);put16(data+2,0x1234);memset(data+4,0x17,257);put16(data+261,0);
    puts("LEGACY_UNALIGNED_UPLOAD");fflush(stdout);reference_upload(a,data);dispose(a);dispose(b);return 0;
  }
  bank_validation();primitive_access();unsigned mismatches=live_high_bytes(a,b,little);unsigned cases=uploads(a,b,little);unsigned brr_cases=brr_decoder(a);
  for(unsigned range=0;range<16;range++)for(unsigned filter=0;filter<4;filter++)(void)synthetic_song(a,range,filter,-1,0,0);
  assert(little || mismatches>0);
  assert(cases==400 && brr_cases==786432 && digest==UINT64_C(0x7ffc2b10517af1ae));
  printf("SPC live playback: all 65536 high-byte values, 65536 RAM word addresses and wraps, %u unaligned upload cases, %u exact mathematical BRR blocks, 64 audible stereo BRR/song scenarios; digest %016llx; pointer width %zu; %s endian; old live-field mismatches %u.\n",cases,brr_cases,(unsigned long long)digest,sizeof(void *),little?"little":"big",mismatches);
  digest=UINT64_C(14695981039346656037);
  const unsigned depths[]={1,127,255},delays[]={0,3,255},rates[]={1,63};
  unsigned tremolo_cases=0;
  for(unsigned d=0;d<3;d++)for(unsigned wait=0;wait<3;wait++)for(unsigned r=0;r<2;r++) {
    uint64_t off=synthetic_song(a,8,0,0,delays[wait],rates[r]);
    uint64_t on=synthetic_song(b,8,0,(int)depths[d],delays[wait],rates[r]);
    if(!((delays[wait]==255 || depths[d]==1) ? off==on : off!=on))fprintf(stderr,"tremolo mismatch depth=%u delay=%u rate=%u off=%016llx on=%016llx\n",depths[d],delays[wait],rates[r],(unsigned long long)off,(unsigned long long)on);
    assert((delays[wait]==255 || depths[d]==1) ? off==on : off!=on);
    tremolo_cases++;
  }
  assert(digest==UINT64_C(0x0273f69d973d4614));
  printf("Full-player tremolo: %u enabled/disabled song pairs, delay and disable commands, PCM/state digest %016llx.\n",tremolo_cases,(unsigned long long)digest);
  dispose(a);dispose(b);return 0;
}
