#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include "xiso.h"
void platform_log(const char *format, ...) { (void)format; }
static void *reader(void *unused) {
    (void)unused;
    for (int i = 0; i < 500; ++i) {
        uint8_t out[32] = {0};
        assert(gamepkg_path_read("/virtual/disc/maps/UI.MAP", 2, out, 30) == 8);
        assert(!memcmp(out, "23456789", 8));
    }
    return NULL;
}
int main(int argc, char **argv) {
    assert(argc == 3);
    char error[512];
    int mounted = xiso_view_mount(argv[1], "/virtual/disc", error, sizeof(error));
    if (!strcmp(argv[2], "bad")) { assert(!mounted); return 0; }
    assert(mounted);
    assert(!xiso_view_mount(argv[1], "/different", error, sizeof(error)));
    assert((fcntl(xiso_view_source_fd(), F_GETFL) & O_ACCMODE) == O_RDONLY);
    assert(write(xiso_view_source_fd(), "x", 1) == -1 && errno == EBADF);
    gamepkg_path_info info;
    assert(gamepkg_path_stat("/virtual/disc/maps/UI.MAP", &info) == 0);
    assert(info.kind == GAMEPKG_PATH_FILE && info.size == 10);
    assert(info.flags & GAMEPKG_PATH_READ_ONLY);
    assert(gamepkg_path_stat("/virtual/disc/maps", &info) == 0);
    assert(info.kind == GAMEPKG_PATH_DIRECTORY);
    assert(xiso_view_lookup("/virtual/disc/maps/../ui.map") == -1);
    assert(xiso_view_lookup("/virtual/disc-other/maps/ui.map") == -2);
    assert(xiso_view_lookup("/virtual/disc/maps/not-there.map") == -1);
    uint8_t out[32] = {0};
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", 9, out, 32) == 1 && out[0] == '9');
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", 10, out, 32) == 0);
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", UINT64_MAX, out, 32) == 0);
    assert(gamepkg_path_read("/virtual/disc/maps/ui.map", 0, NULL, 1) == -1);
    uint32_t h, stale;
    assert(gamepkg_dir_open("/virtual/disc/maps", &h) == 1);
    stale = h;
    assert(gamepkg_dir_read(h, out, sizeof(out)) == 6 && !strcmp((char *)out, "ui.map"));
    assert(gamepkg_dir_read(h, out, sizeof(out)) == 0);
    gamepkg_dir_close(h);
    assert(gamepkg_dir_read(stale, out, sizeof(out)) == -1);
    assert(gamepkg_dir_open("/virtual/disc/maps", &h) == 1 && h != stale);
    gamepkg_dir_close(h);
    pthread_t threads[8];
    for (int i = 0; i < 8; ++i) assert(!pthread_create(&threads[i], NULL, reader, NULL));
    for (int i = 0; i < 8; ++i) assert(!pthread_join(threads[i], NULL));
    return 0;
}
