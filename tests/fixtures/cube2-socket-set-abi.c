/* The pinned Unix ENet ABI uses int tokens. No OS headers are needed here. */
typedef int ENetSocket;
#define ENET_SOCKET_NULL (-1)
#include "../../engines/cube2/adapters/socket_set.h"
_Static_assert(sizeof(ENetSocket)==4, "ENet token must be 32 bits");
_Static_assert(sizeof(ENetSocketSet)==72, "socket set must remain fixed-size");
#ifdef GAMEPKG_EXPECT_POINTER_BITS
_Static_assert(sizeof(void *)*8==GAMEPKG_EXPECT_POINTER_BITS, "wrong executable ABI");
#endif
int main(void)
{
    ENetSocketSet set;
    unsigned i;
    ENET_SOCKETSET_EMPTY(set);
    for(i=0;i<16;i++) ENET_SOCKETSET_ADD(set,1000000+(int)i);
    if(set.count!=16 || !gp_net_set_valid(&set)) return 1;
    ENET_SOCKETSET_ADD(set,2147483647);
    if(set.count!=16 || gp_net_set_valid(&set)) return 2;
    ENET_SOCKETSET_EMPTY(set);
    ENET_SOCKETSET_ADD(set,2147483647); ENET_SOCKETSET_ADD(set,1024);
    if(!ENET_SOCKETSET_CHECK(set,2147483647) || !ENET_SOCKETSET_CHECK(set,1024)) return 3;
    ENET_SOCKETSET_REMOVE(set,2147483647);
    if(ENET_SOCKETSET_CHECK(set,2147483647) || !ENET_SOCKETSET_CHECK(set,1024) || set.count!=1) return 4;
    ENET_SOCKETSET_REMOVE(set,1024);
    if(set.count || !gp_net_set_valid(&set)) return 5;
    ENET_SOCKETSET_ADD(set,-1);
    if(gp_net_set_valid(&set)) return 6;
    return 0;
}
