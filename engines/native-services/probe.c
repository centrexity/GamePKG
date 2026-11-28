/* Asset-free native caller. Game-facing code uses only the GamePKG ABI. */
#include "gamepkg_port.h"
_Static_assert(sizeof(uint64_t)==8,"64-bit IDs");
_Static_assert(sizeof(uint32_t)==4,"32-bit wire fields");
_Static_assert(GAMEPKG_SIMD_ABI==2u,"SIMD helper ABI");
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
int port_client_probe(uint64_t rom) {
    uint8_t reply[64]={0},request[32]={0};
    uint32_t simd_a[4]={1,2,3,4},simd_b[4]={10,20,30,40},simd_out[4]={0};
    float float_a[4]={1.0f,2.0f,3.0f,4.0f},float_b[4]={10.0f,20.0f,30.0f,40.0f},float_out[4]={0};
    gamepkg_simd_u32x4_add(simd_a,simd_b,simd_out);
    CHECK(simd_out[0]==11 && simd_out[1]==22 && simd_out[2]==33 && simd_out[3]==44);
    gamepkg_simd_f32x4_add(float_a,float_b,float_out);
    CHECK(float_out[0]==11.0f && float_out[1]==22.0f && float_out[2]==33.0f && float_out[3]==44.0f);
    gamepkg_simd_f32x4_mul(float_a,float_b,float_out);
    CHECK(float_out[0]==10.0f && float_out[1]==40.0f && float_out[2]==90.0f && float_out[3]==160.0f);
    CHECK(gamepkg_simd_f32x4_sum(float_out)==300.0f);
    {
        uint32_t mask_a[4]={0xf0u,0x0fu,0xaau,0x55u},mask_b[4]={0x0fu,0x10u,0x55u,0xaau},mask_out[4]={0};
        float sub_b[4]={2.0f,4.0f,5.0f,8.0f},mixed[4]={-1.0f,2.0f,-3.0f,4.0f},one[4]={1.0f,2.0f,3.0f,4.0f},extra[4]={0};
        gamepkg_simd_u32x4_sub(simd_b,simd_a,simd_out);
        CHECK(simd_out[0]==9 && simd_out[1]==18 && simd_out[2]==27 && simd_out[3]==36);
        gamepkg_simd_u32x4_or(mask_a,mask_b,mask_out);
        CHECK(mask_out[0]==0xffu && mask_out[1]==0x1fu && mask_out[2]==0xffu && mask_out[3]==0xffu);
        gamepkg_simd_u32x4_shl(simd_a,2,mask_out);
        CHECK(mask_out[0]==4 && mask_out[1]==8 && mask_out[2]==12 && mask_out[3]==16);
        CHECK(gamepkg_simd_u32x4_wrapping_sum(simd_a)==10);
        gamepkg_simd_f32x4_div(float_a,sub_b,float_out);
        CHECK(float_out[0]==0.5f && float_out[1]==0.5f && float_out[2]==0.6f && float_out[3]==0.5f);
        gamepkg_simd_f32x4_abs(mixed,extra);
        CHECK(extra[0]==1.0f && extra[1]==2.0f && extra[2]==3.0f && extra[3]==4.0f);
        gamepkg_simd_f32x4_mul_add(one,float_b,one,extra);
        CHECK(extra[0]==11.0f && extra[1]==42.0f && extra[2]==93.0f && extra[3]==164.0f);
        CHECK(gamepkg_simd_f32x4_dot(one,float_b)==300.0f);
        CHECK(gamepkg_simd_f32x4_min(one)==1.0f && gamepkg_simd_f32x4_max(one)==4.0f);
    }
    CHECK(gamepkg_port_call(GAMEPKG_PORT_VERSION,0,0,reply,4)==4);
    CHECK(gamepkg_port_get_u32(reply)==GAMEPKG_PORT_ABI);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_GRANTS,0,0,reply,64)==48);
    CHECK(gamepkg_port_get_u64(reply)==rom && gamepkg_port_get_u32(reply+8)==3);
    gamepkg_port_put_u64(request,rom);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_FILE_SIZE,request,8,reply,8)==8);
    CHECK(gamepkg_port_get_u64(reply)==((uint64_t)1<<32)+41);
    gamepkg_port_put_u64(request+8,((uint64_t)1<<32)+37);
    gamepkg_port_put_u32(request+16,4);
    /* Request/reply aliasing must not invalidate the encoded request. */
    CHECK(gamepkg_port_call(GAMEPKG_PORT_FILE_READ,request,20,request,4)==4);
    CHECK(request[0]=='d' && request[1]=='i' && request[2]=='s' && request[3]=='c');
    gamepkg_port_put_u32(request,0);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_NETWORK,request,4,reply,4)==GAMEPKG_PORT_DENIED);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_WINDOW_STATE,0,0,reply,12)==12);
    CHECK(gamepkg_port_get_u32(reply)==1000 && gamepkg_port_get_u32(reply+4)==500);
    /* This fixture has no tick service, so VSync preference requests are denied. */
    {
        uint32_t enabled=1u;
        CHECK(gamepkg_window_vsync_preference(&enabled)==GAMEPKG_PORT_DENIED);
        CHECK(enabled==1u);
        CHECK(gamepkg_window_set_vsync(0)==GAMEPKG_PORT_DENIED);
        CHECK(gamepkg_window_set_vsync(1)==GAMEPKG_PORT_DENIED);
        CHECK(gamepkg_window_set_vsync(2)==GAMEPKG_PORT_INVALID);
    }
    {
        uint64_t period=0,deadline=0;
        uint32_t fields=0,callbacks=0,i;
        uint8_t glyph[12+5*7*4];
        uint8_t planes[4*4+4+4],thp_request[8+4*4+4+4],rgba[4*4*4];
        CHECK(gamepkg_vi_cadence(0,&period,&fields)==16);
        CHECK(period==16683350u && fields==2u);
        CHECK(gamepkg_thp_frame(0,100,2,&deadline,&callbacks)==24);
        CHECK(deadline==100u+2u*16683350u && callbacks==1u);
        CHECK(gamepkg_font_glyph(0,(uint32_t)'A',1,glyph,(uint32_t)sizeof glyph)==(int32_t)sizeof glyph);
        CHECK(gamepkg_port_get_u32(glyph)==5u && gamepkg_port_get_u32(glyph+4)==7u);
        CHECK(gamepkg_port_get_u32(glyph+8)==6u);
        for (i=0;i<16u;++i) planes[i]=0u;
        for (i=16u;i<24u;++i) planes[i]=128u;
        CHECK(gamepkg_thp_display(4,4,planes,thp_request,(uint32_t)sizeof thp_request,rgba,(uint32_t)sizeof rgba)==(int32_t)sizeof rgba);
        for (i=0;i<(uint32_t)sizeof rgba;++i) CHECK(rgba[i]==(i%4u==3u ? 255u : 0u));
        /* No gamepad is claimed in this fixture, so an exact frame is denied. */
        CHECK(gamepkg_rumble_frame(0,1,2,1)==GAMEPKG_PORT_DENIED);
    }
    gamepkg_port_put_u32(request,2);request[4]=0;request[5]=128;request[6]=255;request[7]=127;
    CHECK(gamepkg_port_call(GAMEPKG_PORT_AUDIO,request,8,0,0)==0);
    uint64_t stream=0;
    CHECK(gamepkg_port_audio_stream_create_with_rate(96000,1,2,&stream)==GAMEPKG_PORT_UNSUPPORTED);
    CHECK(stream==0);
    CHECK(gamepkg_port_audio_stream_create_with_rate(32000,1,2,&stream)==0 && stream!=0);
    gamepkg_port_put_u64(request,stream);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_AUDIO_STREAM_RELEASE,request,8,0,0)==0);
    CHECK(gamepkg_port_audio_stream_create(1,2,&stream)==0);
    gamepkg_port_put_u64(request,stream);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_AUDIO_STREAM_RELEASE,request,8,0,0)==0);
    CHECK(gamepkg_port_call(0xffffffffu,0,0,0,0)==GAMEPKG_PORT_UNSUPPORTED);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_VERSION,0,1,reply,4)==GAMEPKG_PORT_INVALID);
    return 0;
}
