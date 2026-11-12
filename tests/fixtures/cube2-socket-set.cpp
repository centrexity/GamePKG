// Exercise the exact staged header, including forged shapes and large tokens.
#include <enet/enet.h>
#include <cassert>
#include <climits>
#include <cstdint>
#include <cstdio>
#include <sys/select.h>
int main()
{
    static_assert(sizeof(ENetSocketSet)<=sizeof(fd_set), "virtual sets grew");
    struct { uint32_t before; ENetSocketSet set; uint32_t after; } guarded={0x12345678,{},0x87654321};
    ENetSocketSet &set=guarded.set;
    ENET_SOCKETSET_EMPTY(set);
    for(int i=0;i<16;i++) ENET_SOCKETSET_ADD(set,1000000+i*10003);
    assert(set.count==16 && gp_net_set_valid(&set));
    for(int i=0;i<16;i++) { ENET_SOCKETSET_ADD(set,1000000+i*10003); assert(ENET_SOCKETSET_CHECK(set,1000000+i*10003)); }
    assert(set.count==16 && gp_net_set_valid(&set));
    ENET_SOCKETSET_ADD(set,INT_MAX); assert(set.count==16 && !gp_net_set_valid(&set));
    assert(guarded.before==0x12345678 && guarded.after==0x87654321);
    ENET_SOCKETSET_EMPTY(set); ENET_SOCKETSET_ADD(set,INT_MAX); ENET_SOCKETSET_ADD(set,1024); ENET_SOCKETSET_ADD(set,32);
    ENET_SOCKETSET_REMOVE(set,1024); assert(!ENET_SOCKETSET_CHECK(set,1024) && set.count==2 && gp_net_set_valid(&set));
    ENET_SOCKETSET_REMOVE(set,INT_MAX); ENET_SOCKETSET_REMOVE(set,32); assert(!set.count && gp_net_set_valid(&set));
    for(int cycle=0;cycle<1280;cycle++) {
        ENET_SOCKETSET_ADD(set,1000000+cycle); assert(ENET_SOCKETSET_CHECK(set,1000000+cycle));
        ENET_SOCKETSET_REMOVE(set,1000000+cycle); assert(!set.count && !ENET_SOCKETSET_CHECK(set,1000000+cycle));
    }
    ENET_SOCKETSET_ADD(set,-1); assert(!gp_net_set_valid(&set));
    ENET_SOCKETSET_EMPTY(set); ENET_SOCKETSET_ADD(set,31); assert(!gp_net_set_valid(&set));
    ENET_SOCKETSET_EMPTY(set); set.count=GP_NET_SET_CAPACITY+1;
    assert(!gp_net_set_valid(&set) && !ENET_SOCKETSET_CHECK(set,32));
    ENET_SOCKETSET_REMOVE(set,32); assert(set.invalid);
    ENET_SOCKETSET_EMPTY(set); set.count=2; set.sockets[0]=set.sockets[1]=32; assert(!gp_net_set_valid(&set));
    assert(guarded.before==0x12345678 && guarded.after==0x87654321);
    puts("Bounded opaque socket sets: large IDs, 16-entry capacity, duplicates, removal, forged shapes and canaries passed.");
}
