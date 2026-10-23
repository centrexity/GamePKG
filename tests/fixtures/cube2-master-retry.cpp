// Network calls are recorded here; the included engine functions are unchanged.
#include <cassert>
#include <cctype>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
using ENetSocket = int;
constexpr int ENET_SOCKET_NULL = -1, ENET_HOST_ANY = 0, ENET_PORT_ANY = 0, CON_ERROR = 1;
struct ENetAddress { unsigned host, port; };
struct ENetBuffer { void *data; size_t dataLength; };
template<class T> struct vector {
    std::vector<T> bytes;
    void setsize(int size) { bytes.resize(size); }
    int length() const { return int(bytes.size()); }
    bool empty() const { return bytes.empty(); }
    T *getbuf() { return bytes.data(); }
    T &operator[](int index) { return bytes[index]; }
    void put(const T *data, size_t size) { bytes.insert(bytes.end(), data, data+size); }
};
int totalmillis = 0, allowupdatemaster = 1, serverport = 32123;
char mastername[64] = "private.test";
int connectcalls = 0, destroyed = 0, disconnected = 0, sendlimit = 4096;
bool refused = false;
std::string sent;
std::vector<std::string> messages, forwarded;
void logmessage(const char *format, va_list args) { char text[1024]; vsnprintf(text,sizeof(text),format,args); messages.emplace_back(text); }
void conoutf(const char *format, ...) { va_list args; va_start(args,format); logmessage(format,args); va_end(args); }
void conoutf(int, const char *format, ...) { va_list args; va_start(args,format); logmessage(format,args); va_end(args); }
void logoutf(const char *format, ...) { va_list args; va_start(args,format); logmessage(format,args); va_end(args); }
bool iscubespace(char value) { return std::isspace(static_cast<unsigned char>(value)); }
bool matchstring(const char *input, int size, const char *expected) { return size==int(strlen(expected)) && !memcmp(input,expected,size); }
namespace server {
    void masterdisconnected() { disconnected++; }
    void processmasterinput(const char *input, int size, const char *args) { forwarded.emplace_back(std::string(input,size)+":"+args); }
}
ENetSocket connectmaster(bool wait) { assert(!wait); connectcalls++; return refused ? ENET_SOCKET_NULL : 31+connectcalls; }
void enet_socket_destroy(ENetSocket socket) { assert(socket>=32); destroyed++; }
int enet_socket_send(ENetSocket socket, const ENetAddress *address, const ENetBuffer *buffers, size_t count) {
    assert(socket>=32 && !address && count==1);
    if(sendlimit<0) return -1;
    int size=std::min(int(buffers[0].dataLength),sendlimit);
    sent.append(static_cast<const char *>(buffers[0].data),size); return size;
}
#define defvformatstring(name,format,last) char name[4096]; va_list args; va_start(args,last); vsnprintf(name,sizeof(name),format,args); va_end(args)
#include "original-master.h"
void reset() {
    if(mastersock!=ENET_SOCKET_NULL) disconnectmaster();
    totalmillis=lastupdatemaster=lastconnectmaster=masterconnecting=masterconnected=0;
    masterout.setsize(0); masterin.setsize(0); masteroutpos=masterinpos=0;
    connectcalls=destroyed=disconnected=0; sendlimit=4096; refused=false;
    sent.clear(); messages.clear(); forwarded.clear();
    strcpy(mastername,"private.test"); allowupdatemaster=1;
}
int main() {
    reset(); originalheartbeat();
    assert(connectcalls==1 && mastersock==32 && lastconnectmaster==1 && masterconnecting==1 && lastupdatemaster==1);
    originalheartbeat(); assert(connectcalls==1);
    totalmillis=60000; flushmasteroutput(); assert(mastersock==32 && destroyed==0);
    masteraddress={42,123}; masterin.put("partial",7); masterinpos=2;
    totalmillis=60001; flushmasteroutput();
    assert(mastersock==ENET_SOCKET_NULL && destroyed==1 && disconnected==1);
    assert(lastconnectmaster==1 && !lastupdatemaster && !masterconnecting && !masterconnected);
    assert(masterout.empty() && masterin.empty() && !masteroutpos && !masterinpos && !masteraddress.host && !masteraddress.port);
    for(int now: {300000,300001}) { totalmillis=now; originalheartbeat(); assert(connectcalls==1); }
    totalmillis=300002; originalheartbeat(); assert(connectcalls==2 && mastersock==33 && lastconnectmaster==300002);
    masterconnecting=0; masterconnected=totalmillis;
    sendlimit=0; flushmasteroutput(); assert(!masterout.empty() && !masteroutpos && sent.empty());
    sendlimit=3; while(!masterout.empty()) flushmasteroutput();
    assert(sent=="regserv 32123\n" && !masteroutpos);
    const int registered=lastupdatemaster;
    const char *partial="failreg temporarily unavailable"; masterin.put(partial,strlen(partial)); processmasterinput();
    assert(messages.size()==1 && masterinpos==0); // Only the earlier timeout was logged.
    const char *replies="\nsuccreg\ncustom argument\n"; masterin.put(replies,strlen(replies)); processmasterinput();
    assert(messages.size()==3 && messages[1]=="master server registration failed: temporarily unavailable" && messages[2]=="master server registration succeeded");
    assert(forwarded.size()==1 && forwarded[0]=="custom:argument" && masterin.empty() && !masterinpos);
    assert(mastersock==33 && masterconnected && lastupdatemaster==registered); // failreg does not invent immediate retries.
    totalmillis=registered+3600000; originalheartbeat(); assert(masterout.empty());
    totalmillis++; originalheartbeat(); assert(!masterout.empty() && connectcalls==2);
    sendlimit=-1; flushmasteroutput(); assert(mastersock==ENET_SOCKET_NULL && destroyed==2 && disconnected==2 && lastconnectmaster==300002);
    originalheartbeat(); assert(connectcalls==3); // Five minutes since the connection attempt already elapsed.
    reset(); refused=true; originalheartbeat(); assert(connectcalls==1 && mastersock==ENET_SOCKET_NULL && lastconnectmaster==0 && lastupdatemaster==1);
    totalmillis=3600001; originalheartbeat(); assert(connectcalls==1);
    totalmillis++; refused=false; originalheartbeat(); assert(connectcalls==2 && mastersock!=ENET_SOCKET_NULL);
    reset(); allowupdatemaster=0; originalheartbeat(); assert(connectcalls==0 && lastupdatemaster==1);
    reset(); mastername[0]=0; originalheartbeat(); assert(connectcalls==0 && lastupdatemaster==1);
    reset(); assert(requestmaster(std::string(4096,'x').c_str()));
    assert(masterout.length()==4096 && !requestmaster("not appended") && masterout.length()==4096);
    reset(); puts("Original master timing/parser and output state checks passed.");
}
