// Reviewed ENet transport adaptation. Zlib license; see ../upstream/readme_source.txt.
// All external sockets live in the host; fd 3 is only a bounded RPC channel.
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <arpa/inet.h>

#define GP_NET_MAX 8192u
#define GP_NET_DATAGRAM 4096u
static pthread_mutex_t gp_net_mutex = PTHREAD_MUTEX_INITIALIZER;
static uint32_t gp_net_get32(const unsigned char *p)
{
    return (uint32_t)p[0] | (uint32_t)p[1]<<8 | (uint32_t)p[2]<<16 | (uint32_t)p[3]<<24;
}
static void gp_net_put32(unsigned char *p, uint32_t value)
{
    unsigned i; for(i=0;i<4;i++) p[i]=(unsigned char)(value>>(8*i));
}
static int gp_net_transfer(void *data, size_t size, int output)
{
    unsigned char *p=(unsigned char *)data;
    while(size)
    {
        ssize_t n=output ? write(3,p,size) : read(3,p,size);
        if(n<0 && errno==EINTR) continue;
        if(n<=0) return -1;
        p+=n;size-=(size_t)n;
    }
    return 0;
}
// Returns payload bytes, -1 for denied/failure, -2 for bounded retry.
static int gp_net_call(uint32_t op, const void *request, size_t size, unsigned char *reply, size_t capacity)
{
    unsigned char header[16]; int result=-1;
    if(size>GP_NET_MAX || capacity>GP_NET_MAX) return -1;
    pthread_mutex_lock(&gp_net_mutex);
    memcpy(header,"GPN1",4);gp_net_put32(header+4,op);gp_net_put32(header+8,(uint32_t)size);gp_net_put32(header+12,0);
    if(gp_net_transfer(header,16,1) || gp_net_transfer((void *)request,size,1) || gp_net_transfer(header,16,0)) goto done;
    if(memcmp(header,"GPR2",4) || gp_net_get32(header+12) || gp_net_get32(header+8)>capacity) goto done;
    size=gp_net_get32(header+8);
    if(gp_net_transfer(reply,size,0)) goto done;
    if(gp_net_get32(header+4)==0) result=(int)size;
    else if(gp_net_get32(header+4)==2 && !size) result=-2;
done:
    pthread_mutex_unlock(&gp_net_mutex);
    return result;
}
static void gp_net_address(unsigned char *p, const ENetAddress *address)
{
    // host is already in network byte order; preserve its octets, not its integer.
    memcpy(p,&address->host,4);gp_net_put32(p+4,address->port);
}
static int gp_net_read_address(ENetAddress *address, const unsigned char *p)
{
    uint32_t port=gp_net_get32(p+4); if(port>65535) return -1;
    memcpy(&address->host,p,4); address->port=(enet_uint16)port;return 0;
}
int enet_address_set_host_ip(ENetAddress *address, const char *name)
{
    return inet_pton(AF_INET,name,&address->host)==1 ? 0 : -1;
}
int enet_address_set_host(ENetAddress *address, const char *name)
{
    unsigned char reply[4]; size_t length=strlen(name);
    if(!length || length>253 || gp_net_call(6,name,length,reply,4)!=4) return -1;
    memcpy(&address->host,reply,4);return 0;
}
int enet_address_get_host_ip(const ENetAddress *address, char *name, size_t length)
{
    return inet_ntop(AF_INET,&address->host,name,(socklen_t)length) ? 0 : -1;
}
int enet_address_get_host(const ENetAddress *address, char *name, size_t length)
{
    // Reverse lookup is deliberately numeric; it never starts ambient DNS.
    return enet_address_get_host_ip(address,name,length);
}
ENetSocket enet_socket_create(ENetSocketType type)
{
    unsigned char request[4], reply[4]; uint32_t id;
    gp_net_put32(request,(uint32_t)type);
    if(gp_net_call(1,request,4,reply,4)!=4) return ENET_SOCKET_NULL;
    id=gp_net_get32(reply);if(id<32 || id>=1024) return ENET_SOCKET_NULL;
    return (ENetSocket)id;
}
int enet_socket_bind(ENetSocket socket, const ENetAddress *address)
{
    unsigned char request[12]; ENetAddress any={ENET_HOST_ANY,0};
    gp_net_put32(request,(uint32_t)socket);gp_net_address(request+4,address ? address : &any);
    return gp_net_call(2,request,12,NULL,0)==0 ? 0 : -1;
}
int enet_socket_get_address(ENetSocket socket, ENetAddress *address)
{
    unsigned char request[4], reply[8];gp_net_put32(request,(uint32_t)socket);
    return gp_net_call(5,request,4,reply,8)==8 ? gp_net_read_address(address,reply) : -1;
}
int enet_socket_listen(ENetSocket socket, int backlog)
{
    unsigned char request[8];
    if(socket<32 || socket>=1024 || backlog < -1 || backlog > 128) return -1;
    gp_net_put32(request,(uint32_t)socket);gp_net_put32(request+4,(uint32_t)backlog);
    return gp_net_call(9,request,8,NULL,0)==0 ? 0 : -1;
}
// Only the original synchronous list browser uses the sanitized snapshot path.
ENetSocket gamepkg_master_socket(int snapshot)
{
    unsigned char reply[4];uint32_t id;
    if(!snapshot) return enet_socket_create(ENET_SOCKET_TYPE_STREAM);
    if(gp_net_call(16,NULL,0,reply,4)!=4) return ENET_SOCKET_NULL;
    id=gp_net_get32(reply);return id>=32 && id<1024 ? (ENetSocket)id : ENET_SOCKET_NULL;
}
int enet_socket_set_option(ENetSocket socket, ENetSocketOption option, int value)
{
    if(socket<32 || socket>=1024) return -1;
    unsigned char request[12];
    gp_net_put32(request,(uint32_t)socket);gp_net_put32(request+4,(uint32_t)option);gp_net_put32(request+8,(uint32_t)value);
    return gp_net_call(11,request,sizeof(request),NULL,0)==0 ? 0 : -1;
}
int enet_socket_get_option(ENetSocket socket, ENetSocketOption option, int *value)
{
    unsigned char request[8],reply[4];uint32_t result;
    if(socket<32 || socket>=1024 || !value) return -1;
    gp_net_put32(request,(uint32_t)socket);gp_net_put32(request+4,(uint32_t)option);
    if(gp_net_call(12,request,sizeof(request),reply,sizeof(reply))!=4) return -1;
    result=gp_net_get32(reply);if(result>2147483647u) return -1;
    *value=(int)result;return 0;
}

int enet_socket_connect(ENetSocket socket, const ENetAddress *address)
{
    unsigned char request[12];
    if(socket<32 || socket>=1024 || !address) return -1;
    gp_net_put32(request,(uint32_t)socket);gp_net_address(request+4,address);
    if(gp_net_call(10,request,12,NULL,0)!=0) return -1;
    return 0;
}
ENetSocket enet_socket_accept(ENetSocket socket, ENetAddress *address)
{
    unsigned char request[4],reply[12];uint32_t id;ENetAddress peer;
    if(socket<32 || socket>=1024) return ENET_SOCKET_NULL;
    gp_net_put32(request,(uint32_t)socket);
    if(gp_net_call(14,request,4,reply,12)!=12) return ENET_SOCKET_NULL;
    id=gp_net_get32(reply);
    if(id<32 || id>=1024 || gp_net_read_address(&peer,reply+4)) return ENET_SOCKET_NULL;
    if(address) *address=peer;
    return (ENetSocket)id;
}
int enet_socket_shutdown(ENetSocket socket, ENetSocketShutdown how)
{
    unsigned char request[8];
    if(socket<32 || socket>=1024 || (unsigned)how>2) return -1;
    gp_net_put32(request,(uint32_t)socket);gp_net_put32(request+4,(uint32_t)how);
    return gp_net_call(13,request,8,NULL,0)==0 ? 0 : -1;
}
void enet_socket_destroy(ENetSocket socket)
{
    unsigned char request[4];if(socket<32 || socket>=1024) return;
    gp_net_put32(request,(uint32_t)socket);(void)gp_net_call(7,request,4,NULL,0);
}
int enet_socket_send(ENetSocket socket, const ENetAddress *address, const ENetBuffer *buffers, size_t count)
{
    unsigned char request[12+GP_NET_DATAGRAM],reply[4];size_t length=0,i;int result;
    if(socket<32 || socket>=1024 || count>65 || !buffers) return -1;
    gp_net_put32(request,(uint32_t)socket);
    if(address) gp_net_address(request+4,address); else memset(request+4,0,8);
    for(i=0;i<count;i++)
    {
        if(buffers[i].dataLength>GP_NET_DATAGRAM-length) return -1;
        if(buffers[i].dataLength) memcpy(request+12+length,buffers[i].data,buffers[i].dataLength);
        length+=buffers[i].dataLength;
    }
    result=gp_net_call(3,request,12+length,reply,4);
    if(result==-2) return 0;
    if(result!=4 || gp_net_get32(reply)>length) return -1;
    return (int)gp_net_get32(reply);
}
int enet_socket_receive(ENetSocket socket, ENetAddress *address, ENetBuffer *buffers, size_t count)
{
    unsigned char request[8],reply[8+GP_NET_DATAGRAM];size_t capacity=0,i,offset=8,left;int result;
    if(socket<32 || socket>=1024 || count>65 || !buffers) return -1;
    // ENet may offer a growing TCP-list buffer. Read only one bounded chunk.
    for(i=0;i<count;i++) { size_t room=GP_NET_DATAGRAM-capacity;capacity+=buffers[i].dataLength<room ? buffers[i].dataLength : room; }
    if(!capacity) return -1;
    gp_net_put32(request,(uint32_t)socket);gp_net_put32(request+4,(uint32_t)capacity);
    result=gp_net_call(4,request,8,reply,sizeof(reply));
    if(result==-2) return 0;
    if(result<8 || (size_t)result-8>capacity) return -1;
    if(address && gp_net_read_address(address,reply)) return -1;
    left=(size_t)result-8;
    for(i=0;i<count && left;i++) { size_t n=buffers[i].dataLength<left ? buffers[i].dataLength : left;memcpy(buffers[i].data,reply+offset,n);offset+=n;left-=n; }
    return result-8;
}
int enet_socket_wait(ENetSocket socket, enet_uint32 *condition, enet_uint32 timeout)
{
    unsigned char request[8],reply[4];int result;
    gp_net_put32(request,(uint32_t)socket);gp_net_put32(request+4,*condition&3);
    result=gp_net_call(8,request,8,reply,4);
    if(result==-2) { *condition=0;return 0; }
    if(result!=4) return -1;
    *condition=gp_net_get32(reply)&3;
    if(!*condition && timeout) { struct timespec delay={0,(long)(timeout>10 ? 10 : timeout)*1000000};nanosleep(&delay,NULL); }
    return 0;
}
int enet_socketset_select(ENetSocket maxSocket, ENetSocketSet *reads, ENetSocketSet *writes, enet_uint32 timeout)
{
    ENetSocket i;int ready=0;(void)timeout;
    if(maxSocket>=1024) return -1;
    for(i=0;i<=maxSocket;i++)
    {
        enet_uint32 wanted=(reads && FD_ISSET(i,reads) ? 2 : 0) | (writes && FD_ISSET(i,writes) ? 1 : 0), actual=wanted;
        if(!wanted) continue;
        if(enet_socket_wait(i,&actual,0)<0) return -1;
        if(reads && !(actual&2)) FD_CLR(i,reads);
        if(writes && !(actual&1)) FD_CLR(i,writes);
        if(actual) ready++;
    }
    return ready;
}
