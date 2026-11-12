/* Private-media integration gate. Reads the player's ISO in place. */
#include "media.h"
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <pthread.h>
#include <stdlib.h>
#ifdef CHECK_POSIX
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
static void check_posix(void) {
    FILE *f = fopen("/gamepkg-disc/default.xbe", "rb");
    assert(f);
    char magic[4];
    assert(fread(magic, 1, 4, f) == 4 && !memcmp(magic, "XBEH", 4));
    assert(fseek(f, 0, SEEK_END) == 0 && ftell(f) > 4);
    assert(fseek(f, 0, SEEK_SET) == 0);
    assert(fread(magic, 1, 4, f) == 4 && !memcmp(magic, "XBEH", 4));
    assert(fclose(f) == 0);
    assert(!fopen("/gamepkg-disc/default.xbe", "wb") && errno == EROFS);
    int fd = open("Burnout 3 Takedown/default.xbe", O_RDONLY);
    assert(fd >= 0);
    struct stat st;
    assert(fstat(fd, &st) == 0 && S_ISREG(st.st_mode));
    assert(read(fd, magic, 4) == 4 && !memcmp(magic, "XBEH", 4));
    assert(lseek(fd, 0, SEEK_SET) == 0);
    assert(st.st_size > 65536 && st.st_size < 8 * 1024 * 1024);
    uint8_t *all = malloc((size_t)st.st_size + 1);
    assert(all);
    assert(read(fd, all, (size_t)st.st_size + 1) == st.st_size);
    assert(!memcmp(all, "XBEH", 4));
    assert(read(fd, all, 1) == 0);
    free(all);
    assert(write(fd, magic, 4) == -1 && errno == EROFS);
    assert(close(fd) == 0);
    assert(open("/gamepkg-disc/../default.xbe", O_RDONLY) == -1 && errno == EINVAL);
    DIR *dir = opendir("/gamepkg-disc");
    assert(dir);
    unsigned count = 0;
    while (readdir(dir)) count++;
    assert(count > 1 && closedir(dir) == 0);
    /* Host paths still use the original libc call; this adapter is not isolation. */
    assert(stat("/dev/null", &st) == 0 && S_ISCHR(st.st_mode));
}
#endif

static uint64_t open_path(const char *path) {
    uint64_t handle = 0;
    assert(gp_media_open((const uint8_t *)path, strlen(path), &handle) == 0);
    assert(handle != 0);
    return handle;
}
static gp_media_info stat_path(const char *path) {
    gp_media_info info;
    assert(gp_media_stat((const uint8_t *)path, strlen(path), &info) == 0);
    assert(info.reserved == 0);
    return info;
}
static unsigned files, directories;
static uint64_t bytes;
static void walk(const char *path) {
    uint64_t handle = open_path(path);
    directories++;
    for (;;) {
        uint8_t name[512];
        size_t length = 999;
        gp_media_info info;
        assert(gp_media_readdir(handle, name, sizeof name, &length, &info) == 0);
        if (!length) break;
        assert(length < sizeof name);
        name[length] = 0;
        char child[512];
        int written = snprintf(child, sizeof child, "%s%s%s", path, *path ? "/" : "", name);
        assert(written >= 0 && (size_t)written < sizeof child);
        gp_media_info actual = stat_path(child);
        assert(actual.size == info.size && actual.is_directory == info.is_directory);
        if (info.is_directory) walk(child);
        else { files++; bytes += info.size; }
    }
    assert(gp_media_close(handle) == 0);
}
static void *read_xbe(void *unused) {
    (void)unused;
    uint64_t handle = open_path("DEFAULT.XBE");
    for (unsigned i = 0; i < 200; i++) {
        uint64_t pos = 999;
        assert(gp_media_seek(handle, 0, 0, &pos) == 0 && pos == 0);
        uint8_t data[4] = {0};
        size_t length = 999;
        assert(gp_media_read(handle, data, sizeof data, &length) == 0);
        assert(length == 4 && memcmp(data, "XBEH", 4) == 0);
        assert(gp_media_seek(handle, -5, 1, &pos) == -EINVAL);
        assert(gp_media_seek(handle, 0, 1, &pos) == 0 && pos == 4);
    }
    assert(gp_media_close(handle) == 0);
    return NULL;
}
int main(int argc, char **argv) {
    if (argc != 2) { fprintf(stderr, "usage: %s /path/to/disc.iso\n", argv[0]); return 2; }
    assert(gp_media_init((const uint8_t *)argv[1], strlen(argv[1]), 64u * 1024u * 1024u) == 0);
#ifdef CHECK_POSIX
    check_posix();
#endif
    assert(gp_media_init((const uint8_t *)argv[1], strlen(argv[1]), 64u * 1024u * 1024u) == -EBUSY);
    uint64_t rejected = 1234;
    assert(gp_media_open((const uint8_t *)"../default.xbe", 14, &rejected) == -EINVAL);
    assert(rejected == 1234);
    uint64_t root = open_path("");
    uint8_t name[512];
    size_t length = 987;
    gp_media_info info = {876, 765, 654};
    assert(gp_media_readdir(root, NULL, 0, &length, &info) == -ENOBUFS);
    assert(length == 987 && info.size == 876);
    assert(gp_media_readdir(root, name, sizeof name, &length, &info) == 0 && length > 0);
    assert(gp_media_read(root, name, sizeof name, &length) == -EISDIR);
    assert(gp_media_shutdown() == -EBUSY);
    assert(gp_media_close(root) == 0);
    assert(gp_media_close(root) == -EBADF);
    walk("");
    assert(files == 716 && directories == 56 && bytes == UINT64_C(2476107072));
    uint64_t handles[128];
    for (unsigned i = 0; i < 128; i++) handles[i] = open_path("default.xbe");
    assert(gp_media_open((const uint8_t *)"default.xbe", 11, &rejected) == -EMFILE);
    for (unsigned i = 0; i < 128; i++) assert(gp_media_close(handles[i]) == 0);
    uint64_t fresh = open_path("default.xbe");
    assert(fresh > handles[127]);
    assert(gp_media_fstat(handles[127], &info) == -EBADF);
    assert(gp_media_close(fresh) == 0);
    pthread_t threads[4];
    for (unsigned i = 0; i < 4; i++) assert(pthread_create(&threads[i], NULL, read_xbe, NULL) == 0);
    for (unsigned i = 0; i < 4; i++) assert(pthread_join(threads[i], NULL) == 0);
    assert(gp_media_shutdown() == 0);
    assert(gp_media_close(fresh) == -ENODEV);
    /* IDs must not alias an old handle after a fresh admission. */
    assert(gp_media_init((const uint8_t *)argv[1], strlen(argv[1]), 64u * 1024u * 1024u) == 0);
    uint64_t newer = open_path("default.xbe");
    assert(newer > fresh);
    assert(gp_media_fstat(fresh, &info) == -EBADF);
    assert(gp_media_close(newer) == 0);
    assert(gp_media_shutdown() == 0);
    printf("C ABI: %u files, %u directories, %llu bytes; bounded handles and concurrent reads passed\n",
           files, directories, (unsigned long long)bytes);
    return 0;
}
