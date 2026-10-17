// Test the production ENet shim's bounded decoding and concurrent RPC framing.
#include <sys/types.h>
#include <sys/socket.h>
#include <unistd.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#include <time.h>
#include "../engines/cube2/upstream/enet/include/enet/enet.h"
#include "../engines/cube2/adapters/syscall_guard.h"
#include "../engines/cube2/adapters/network.h"
struct job { int values[16]; };
static void *requests(void *raw)
{
    job *j=static_cast<job *>(raw);
    for(int i=0;i<16;i++) j->values[i]=enet_socket_create(ENET_SOCKET_TYPE_DATAGRAM);
    return NULL;
}
int main(int argc,char **argv)
{
    if(argc!=2) return 2;
    gamepkg_lockdown();
    errno=0;
    if(socket(AF_INET,SOCK_DGRAM,0)!=-1 || errno!=EPERM) return 3;
    if(!strcmp(argv[1],"create")) printf("%d\n",enet_socket_create(ENET_SOCKET_TYPE_DATAGRAM));
    else if(!strcmp(argv[1],"receive"))
    {
        unsigned char bytes[80];memset(bytes,0xaa,sizeof(bytes));
        ENetBuffer buffer={bytes+16,32};ENetAddress address;
        int result=enet_socket_receive(32,&address,&buffer,1);
        for(int i=0;i<16;i++) if(bytes[i]!=0xaa || bytes[i+48]!=0xaa) return 4;
        printf("%d\n",result);
    }
    else if(!strcmp(argv[1],"large-receive"))
    {
        unsigned char bytes[8192+32];memset(bytes,0xaa,sizeof(bytes));
        ENetBuffer buffer={bytes+16,8192};ENetAddress address;
        int result=enet_socket_receive(32,&address,&buffer,1);
        if(result!=4096) return 9;
        for(int i=0;i<16;i++) if(bytes[i]!=0xaa) return 10;
        for(int i=16;i<4112;i++) if(bytes[i]!='z') return 11;
        for(size_t i=4112;i<sizeof(bytes);i++) if(bytes[i]!=0xaa) return 12;
        printf("%d\n",result);
    }
    else if(!strcmp(argv[1],"oversized"))
    {
        unsigned char one=0; ENetBuffer buffer={&one,4097}; ENetAddress address={0,1234};
        if(enet_socket_send(32,&address,&buffer,1)!=-1) return 5;
    }
    else if(!strcmp(argv[1],"threads"))
    {
        pthread_t threads[4]; job jobs[4]; bool seen[1024]={};
        for(int i=0;i<4;i++) if(pthread_create(&threads[i],NULL,requests,&jobs[i])) return 6;
        for(int i=0;i<4;i++) pthread_join(threads[i],NULL);
        for(int i=0;i<4;i++) for(int j=0;j<16;j++)
        {
            int id=jobs[i].values[j];if(id<32 || id>=1024 || seen[id]) return 7;seen[id]=true;
        }
    }
    else return 8;
    return 0;
}
