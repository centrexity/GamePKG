// Reviewed bounded ENet virtual socket sets; no OS descriptor indexing.
// Zlib license; see ../upstream/readme_source.txt.
#ifndef GAMEPKG_ENET_SOCKET_SET_H
#define GAMEPKG_ENET_SOCKET_SET_H
#define GP_NET_SET_CAPACITY 16u
typedef struct {
    ENetSocket sockets[GP_NET_SET_CAPACITY];
    unsigned count, invalid;
} ENetSocketSet;
static inline void gp_net_set_empty(ENetSocketSet *set)
{
    unsigned i; set->count=0; set->invalid=0;
    for(i=0;i<GP_NET_SET_CAPACITY;i++) set->sockets[i]=ENET_SOCKET_NULL;
}
static inline int gp_net_set_valid(const ENetSocketSet *set)
{
    unsigned i,j;
    if(set->invalid || set->count>GP_NET_SET_CAPACITY) return 0;
    for(i=0;i<set->count;i++) {
        if(set->sockets[i]<32) return 0;
        for(j=0;j<i;j++) if(set->sockets[i]==set->sockets[j]) return 0;
    }
    return 1;
}
static inline int gp_net_set_check(const ENetSocketSet *set, ENetSocket socket)
{
    unsigned i; if(set->count>GP_NET_SET_CAPACITY) return 0;
    for(i=0;i<set->count;i++) if(set->sockets[i]==socket) return 1;
    return 0;
}
static inline void gp_net_set_add(ENetSocketSet *set, ENetSocket socket)
{
    if(socket<32 || !gp_net_set_valid(set)) { set->invalid=1; return; }
    if(gp_net_set_check(set,socket)) return;
    if(set->count==GP_NET_SET_CAPACITY) { set->invalid=1; return; }
    set->sockets[set->count++]=socket;
}
static inline void gp_net_set_remove(ENetSocketSet *set, ENetSocket socket)
{
    unsigned i;
    if(!gp_net_set_valid(set)) { set->invalid=1; return; }
    for(i=0;i<set->count;i++) if(set->sockets[i]==socket) {
        for(;i+1<set->count;i++) set->sockets[i]=set->sockets[i+1];
        set->sockets[--set->count]=ENET_SOCKET_NULL; return;
    }
}
#define ENET_SOCKETSET_EMPTY(set) gp_net_set_empty(&(set))
#define ENET_SOCKETSET_ADD(set,socket) gp_net_set_add(&(set),(socket))
#define ENET_SOCKETSET_REMOVE(set,socket) gp_net_set_remove(&(set),(socket))
#define ENET_SOCKETSET_CHECK(set,socket) gp_net_set_check(&(set),(socket))
#endif
