// Exercise the production persistence adapter without game media or SDL.
#include <unistd.h>
#include <stdint.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../engines/cube2/adapters/syscall_guard.h"
static bool gamepkg_transfer(int fd, void *data, size_t length, bool output)
{
    unsigned char *p = static_cast<unsigned char *>(data);
    while(length)
    {
        ssize_t n = output ? write(fd, p, length) : read(fd, p, length);
        if(n<0 && errno==EINTR) continue;
        if(n<=0) return false;
        p += n; length -= n;
    }
    return true;
}
static uint32_t gamepkg_le32(const unsigned char *p)
{
    return uint32_t(p[0]) | uint32_t(p[1])<<8 | uint32_t(p[2])<<16 | uint32_t(p[3])<<24;
}
static void gamepkg_put32(unsigned char *p, uint32_t n)
{
    for(int i=0;i<4;i++) p[i] = (n>>(8*i))&255;
}
#include "../engines/cube2/adapters/persistence.h"
int main(int argc, char **argv)
{
    gamepkg_lockdown();
    if(argc!=2) return 2;
    if(!strcmp(argv[1], "restore"))
    {
        unsigned char header[16];
        if(!gamepkg_transfer(0, header, 16, false) || memcmp(header, "GPB2", 4)) return 2;
        gamepkg_restore_tree(header);
    }
    else if(strcmp(argv[1], "collect")) return 2;
    gamepkg_tree_snapshot(1);
    return 0;
}
