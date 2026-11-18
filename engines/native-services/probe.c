/* Asset-free native caller. Game-facing code uses only the GamePKG ABI. */
#include "gamepkg_port.h"
_Static_assert(sizeof(uint64_t)==8,"64-bit IDs");
_Static_assert(sizeof(uint32_t)==4,"32-bit wire fields");
#define CHECK(x) do { if (!(x)) return __LINE__; } while(0)
int port_client_probe(uint64_t rom) {
    uint8_t reply[64]={0},request[32]={0};
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
    gamepkg_port_put_u32(request,2);request[4]=0;request[5]=128;request[6]=255;request[7]=127;
    CHECK(gamepkg_port_call(GAMEPKG_PORT_AUDIO,request,8,0,0)==0);
    CHECK(gamepkg_port_call(0xffffffffu,0,0,0,0)==GAMEPKG_PORT_UNSUPPORTED);
    CHECK(gamepkg_port_call(GAMEPKG_PORT_VERSION,0,1,reply,4)==GAMEPKG_PORT_INVALID);
    return 0;
}
