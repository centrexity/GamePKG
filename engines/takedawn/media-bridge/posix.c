/* Linux engine adapter. Link only the reviewed native engine with --wrap for
 * the functions below. This is a media transport, not a process sandbox.
 * Everything outside the two explicit disc roots keeps its original behavior;
 * the GamePKG launcher must separately isolate saves/config/network/processes.
 */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "media.h"
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/statvfs.h>
#include <unistd.h>

#define FD_BASE 0x40000000
#define SLOTS 128
static uint64_t files[SLOTS];
static struct directory { uint64_t handle; struct dirent entry; } dirs[SLOTS];
static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static const char *relative(const char *path) {
    if (!path) return NULL;
    const char *roots[] = {"/gamepkg-disc", "Burnout 3 Takedown"};
    for (unsigned i = 0; i < sizeof roots / sizeof *roots; i++) {
        size_t n = strlen(roots[i]);
        if (!strncmp(path, roots[i], n) && (!path[n] || path[n] == '/'))
            return path + n + (path[n] == '/');
    }
    return NULL;
}
static int error(int32_t result) { if (result < 0) { errno = -result; return -1; } return 0; }
static int slot(int fd) { return fd >= FD_BASE && fd < FD_BASE + SLOTS ? fd - FD_BASE : -1; }
static void metadata(struct stat *st, gp_media_info info) {
    memset(st, 0, sizeof *st);
    st->st_mode = info.is_directory ? S_IFDIR | 0555 : S_IFREG | 0444;
    st->st_nlink = 1;
    st->st_size = (off_t)info.size;
    st->st_blksize = 2048;
    st->st_blocks = (blkcnt_t)((info.size + 511) / 512);
}
extern int __real_open(const char *, int, ...);
int __wrap_open(const char *path, int flags, ...) {
    mode_t mode = 0;
    if ((flags & O_CREAT) || (flags & O_TMPFILE) == O_TMPFILE) {
        va_list ap; va_start(ap, flags); mode = (mode_t)va_arg(ap, int); va_end(ap);
    }
    const char *p = relative(path);
    if (!p) return __real_open(path, flags, mode);
    if ((flags & O_ACCMODE) != O_RDONLY || (flags & (O_CREAT | O_TRUNC | O_APPEND)) || (flags & O_TMPFILE) == O_TMPFILE) {
        errno = EROFS; return -1;
    }
    gp_media_info info;
    if (error(gp_media_stat((const uint8_t *)p, strlen(p), &info))) return -1;
    if ((flags & O_DIRECTORY) && !info.is_directory) { errno = ENOTDIR; return -1; }
    pthread_mutex_lock(&lock);
    int i;
    for (i = 0; i < SLOTS && files[i]; i++);
    int result = -1;
    if (i == SLOTS) errno = EMFILE;
    else if (!error(gp_media_open((const uint8_t *)p, strlen(p), &files[i]))) result = FD_BASE + i;
    pthread_mutex_unlock(&lock);
    return result;
}
extern int __real_close(int);
int __wrap_close(int fd) {
    int i = slot(fd); if (i < 0) return __real_close(fd);
    pthread_mutex_lock(&lock);
    int result = error(gp_media_close(files[i]));
    if (!result) files[i] = 0;
    pthread_mutex_unlock(&lock); return result;
}
extern ssize_t __real_read(int, void *, size_t);
ssize_t __wrap_read(int fd, void *buf, size_t size) {
    int i = slot(fd); if (i < 0) return __real_read(fd, buf, size);
    pthread_mutex_lock(&lock);
    size_t total = 0;
    int rc;
    do {
        size_t count = 0;
        rc = error(gp_media_read(files[i], total ? (uint8_t *)buf + total : buf, size - total, &count));
        if (rc || !count) break;
        total += count;
    } while (total < size);
    pthread_mutex_unlock(&lock); return rc && !total ? -1 : (ssize_t)total;
}
extern ssize_t __real_write(int, const void *, size_t);
ssize_t __wrap_write(int fd, const void *buf, size_t size) {
    if (slot(fd) < 0) return __real_write(fd, buf, size);
    errno = EROFS; return -1;
}
extern off_t __real_lseek(int, off_t, int);
off_t __wrap_lseek(int fd, off_t offset, int whence) {
    int i = slot(fd); if (i < 0) return __real_lseek(fd, offset, whence);
    pthread_mutex_lock(&lock);
    uint64_t position = 0;
    int rc = error(gp_media_seek(files[i], offset, whence, &position));
    pthread_mutex_unlock(&lock); return rc ? -1 : (off_t)position;
}
extern int __real_fstat(int, struct stat *);
int __wrap_fstat(int fd, struct stat *st) {
    int i = slot(fd); if (i < 0) return __real_fstat(fd, st);
    pthread_mutex_lock(&lock);
    gp_media_info info;
    int rc = error(gp_media_fstat(files[i], &info));
    if (!rc) metadata(st, info);
    pthread_mutex_unlock(&lock); return rc;
}
extern int __real_stat(const char *, struct stat *);
int __wrap_stat(const char *path, struct stat *st) {
    const char *p = relative(path); if (!p) return __real_stat(path, st);
    gp_media_info info;
    if (error(gp_media_stat((const uint8_t *)p, strlen(p), &info))) return -1;
    metadata(st, info); return 0;
}
extern int __real_lstat(const char *, struct stat *);
int __wrap_lstat(const char *path, struct stat *st) {
    if (!relative(path)) return __real_lstat(path, st);
    return __wrap_stat(path, st);
}
extern int __real_access(const char *, int);
int __wrap_access(const char *path, int mode) {
    if (!relative(path)) return __real_access(path, mode);
    struct stat st;
    if (__wrap_stat(path, &st)) return -1;
    if (mode & ~(R_OK | W_OK | X_OK)) { errno = EINVAL; return -1; }
    if ((mode & W_OK) || ((mode & X_OK) && !S_ISDIR(st.st_mode))) { errno = EACCES; return -1; }
    return 0;
}
extern DIR *__real_opendir(const char *);
DIR *__wrap_opendir(const char *path) {
    const char *p = relative(path); if (!p) return __real_opendir(path);
    gp_media_info info;
    if (error(gp_media_stat((const uint8_t *)p, strlen(p), &info))) return NULL;
    if (!info.is_directory) { errno = ENOTDIR; return NULL; }
    pthread_mutex_lock(&lock);
    struct directory *dir = NULL;
    for (unsigned i = 0; i < SLOTS; i++) if (!dirs[i].handle) { dir = &dirs[i]; break; }
    if (!dir) errno = EMFILE;
    else if (error(gp_media_open((const uint8_t *)p, strlen(p), &dir->handle))) dir = NULL;
    pthread_mutex_unlock(&lock); return (DIR *)dir;
}
static struct directory *directory(DIR *dir) {
    for (unsigned i = 0; i < SLOTS; i++) if (dir == (DIR *)&dirs[i]) return &dirs[i];
    return NULL;
}
extern struct dirent *__real_readdir(DIR *);
struct dirent *__wrap_readdir(DIR *dir) {
    struct directory *d = directory(dir); if (!d) return __real_readdir(dir);
    pthread_mutex_lock(&lock);
    size_t length = 0;
    gp_media_info info;
    int rc = error(gp_media_readdir(d->handle, (uint8_t *)d->entry.d_name,
                                   sizeof d->entry.d_name - 1, &length, &info));
    if (!rc && length) {
        d->entry.d_name[length] = 0;
        d->entry.d_type = info.is_directory ? DT_DIR : DT_REG;
        d->entry.d_reclen = sizeof d->entry;
        d->entry.d_ino = 1;
    }
    pthread_mutex_unlock(&lock);
    return rc || !length ? NULL : &d->entry;
}
extern int __real_closedir(DIR *);
int __wrap_closedir(DIR *dir) {
    struct directory *d = directory(dir); if (!d) return __real_closedir(dir);
    pthread_mutex_lock(&lock);
    int rc = error(gp_media_close(d->handle)); if (!rc) d->handle = 0;
    pthread_mutex_unlock(&lock); return rc;
}
static ssize_t cookie_read(void *cookie, char *buf, size_t size) {
    return __wrap_read((int)(intptr_t)cookie, buf, size);
}
static int cookie_seek(void *cookie, off64_t *offset, int whence) {
    off_t pos = __wrap_lseek((int)(intptr_t)cookie, *offset, whence);
    if (pos < 0) return -1;
    *offset = pos; return 0;
}
static int cookie_close(void *cookie) { return __wrap_close((int)(intptr_t)cookie); }
extern FILE *__real_fopen(const char *, const char *);
FILE *__wrap_fopen(const char *path, const char *mode) {
    if (!relative(path)) return __real_fopen(path, mode);
    if (!mode || *mode != 'r' || strpbrk(mode, "+wax")) { errno = EROFS; return NULL; }
    int fd = __wrap_open(path, O_RDONLY);
    if (fd < 0) return NULL;
    struct stat st;
    if (__wrap_fstat(fd, &st) || S_ISDIR(st.st_mode)) {
        __wrap_close(fd); errno = EISDIR; return NULL;
    }
    cookie_io_functions_t io = { .read = cookie_read, .write = NULL, .seek = cookie_seek, .close = cookie_close };
    FILE *f = fopencookie((void *)(intptr_t)fd, mode, io);
    if (!f) { int e = errno; __wrap_close(fd); errno = e; }
    return f;
}
