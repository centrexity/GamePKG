/* Maintainer-only exact C/Rust local ABI probe. Never game source. */
#include "gamepkg_port.h"
#include <string.h>
int32_t mutex_c_query(uint64_t id,uint32_t locked,uint32_t owned,uint32_t abandoned,uint32_t depth,int32_t error) {
    uint8_t request[9],storage[18];
    gamepkg_port_put_u64(request,id);request[8]=0;
    if(error!=GAMEPKG_PORT_DENIED) {
    for(uint32_t length=0;length<8;length++) {
        memset(storage,0xa5,sizeof(storage));
        if(gamepkg_local_call(GAMEPKG_LOCAL_MUTEX_STATE,request,length,storage+1,16)!=GAMEPKG_PORT_INVALID)return -101;
        for(uint32_t i=0;i<18;i++)if(storage[i]!=0xa5)return -102;
    }
    memset(storage,0xa5,sizeof(storage));
    if(gamepkg_local_call(GAMEPKG_LOCAL_MUTEX_STATE,request,9,storage+1,16)!=GAMEPKG_PORT_INVALID)return -103;
    for(uint32_t i=0;i<18;i++)if(storage[i]!=0xa5)return -104;
    for(uint32_t capacity=0;capacity<16;capacity++) {
        memset(storage,0xa5,sizeof(storage));
        if(gamepkg_local_call(GAMEPKG_LOCAL_MUTEX_STATE,request,8,storage+1,capacity)!=GAMEPKG_PORT_INVALID)return -105;
        for(uint32_t i=0;i<18;i++)if(storage[i]!=0xa5)return -106;
    }
    }
    memset(storage,0xa5,sizeof(storage));
    int32_t result=gamepkg_local_call(GAMEPKG_LOCAL_MUTEX_STATE,request,8,storage+1,16);
    if(error) {
        if(result!=error)return -107;
        for(uint32_t i=0;i<18;i++)if(storage[i]!=0xa5)return -108;
        return 0;
    }
    if(result!=16 || storage[0]!=0xa5 || storage[17]!=0xa5)return -109;
    if(gamepkg_port_get_u32(storage+1)!=locked || gamepkg_port_get_u32(storage+5)!=owned || gamepkg_port_get_u32(storage+9)!=abandoned || gamepkg_port_get_u32(storage+13)!=depth)return -110;
    return 0;
}
int32_t mutex_c_retire(uint64_t id,uint32_t kind,int32_t expected) {
    uint8_t request[13],output=0xa5;
    gamepkg_port_put_u64(request,id);gamepkg_port_put_u32(request+8,kind);request[12]=0;
    for(uint32_t length=0;length<12;length++) {
        if(gamepkg_local_call(GAMEPKG_LOCAL_SYNC_RELEASE,request,length,&output,0)!=GAMEPKG_PORT_INVALID || output!=0xa5)return -111;
    }
    if(gamepkg_local_call(GAMEPKG_LOCAL_SYNC_RELEASE,request,13,&output,0)!=GAMEPKG_PORT_INVALID || output!=0xa5)return -112;
    if(gamepkg_local_call(GAMEPKG_LOCAL_SYNC_RELEASE,request,12,&output,0)!=expected || output!=0xa5)return -113;
    return 0;
}
