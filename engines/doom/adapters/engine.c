/* GamePKG sandbox adapter for doomgeneric. GPL-2.0-or-later. */
#include "doomgeneric.h"
#include "doomkeys.h"
#include "doomstat.h"
#include "g_game.h"
#include "i_sound.h"
#include "w_wad.h"
#include "z_zone.h"
#include <errno.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <wasi/api.h>
#define EXPORT(name) __attribute__((export_name(name)))
#define SAVE_MAX (512 * 1024)
static uint8_t *wad;
static uint32_t wad_length;
static uint8_t save_data[SAVE_MAX], temp_data[SAVE_MAX];
static uint32_t save_length, temp_length;
static int dirty;
static char diagnostics[4096];
static uint32_t diagnostic_length;
static uint32_t ticks = 1, previous, buttons, accumulator;
static unsigned keyqueue[64], qr, qw;
static int16_t pcm[800];
/* stdio is an entirely private virtual filesystem, never a host descriptor. */
typedef struct {
  uint8_t *data;
  uint32_t size, capacity, pos;
  int used, writing, kind;
} File;
static File files[8];
static File *file(FILE *stream) {
  for (int i = 0; i < 8; i++)
    if (stream == (FILE *)&files[i] && files[i].used)
      return &files[i];
  return NULL;
}
static int kind(const char *path) {
  if (!strcmp(path, "doom.wad"))
    return 1;
  if (!strcmp(path, "./.savegame/temp.dsg"))
    return 2;
  if (!strcmp(path, "./.savegame/doomsav0.dsg"))
    return 3;
  return 0;
}
FILE *__wrap_fopen(const char *path, const char *mode) {
  int k = kind(path), writing = strchr(mode, 'w') != NULL;
  if (!k || (k == 1 && writing) || (k == 3 && !writing && !save_length)) {
    errno = ENOENT;
    return NULL;
  }
  for (int i = 0; i < 8; i++)
    if (!files[i].used) {
      File *f = &files[i];
      *f = (File){.used = 1, .kind = k, .writing = writing};
      f->data = k == 1 ? wad : k == 2 ? temp_data : save_data;
      f->size = writing  ? 0
                : k == 1 ? wad_length
                : k == 2 ? temp_length
                         : save_length;
      f->capacity = k == 1 ? wad_length : SAVE_MAX;
      return (FILE *)f;
    }
  errno = EMFILE;
  return NULL;
}
int __wrap_fclose(FILE *s) {
  File *f = file(s);
  if (!f)
    return EOF;
  if (f->writing) {
    if (f->kind == 2)
      temp_length = f->size;
    else if (f->kind == 3) {
      save_length = f->size;
      dirty = 1;
    }
  }
  f->used = 0;
  return 0;
}
size_t __wrap_fread(void *out, size_t size, size_t count, FILE *s) {
  File *f = file(s);
  if (!f || f->writing || !size || count > SIZE_MAX / size)
    return 0;
  size_t n = size * count;
  if (n > f->size - f->pos)
    n = f->size - f->pos;
  memcpy(out, f->data + f->pos, n);
  f->pos += n;
  return n / size;
}
size_t __wrap_fwrite(const void *in, size_t size, size_t count, FILE *s) {
  File *f = file(s);
  if (!f || !f->writing || !size || count > SIZE_MAX / size)
    return 0;
  size_t n = size * count;
  if (n > f->capacity - f->pos) {
    errno = ENOSPC;
    return 0;
  }
  memcpy(f->data + f->pos, in, n);
  f->pos += n;
  if (f->pos > f->size)
    f->size = f->pos;
  return count;
}
int __wrap_fseek(FILE *s, long offset, int whence) {
  File *f = file(s);
  if (!f)
    return -1;
  int64_t n = offset + (whence == SEEK_CUR   ? f->pos
                        : whence == SEEK_END ? f->size
                                             : 0);
  if (n < 0 || n > f->size)
    return -1;
  f->pos = n;
  return 0;
}
long __wrap_ftell(FILE *s) {
  File *f = file(s);
  return f ? (long)f->pos : -1;
}
int __wrap_feof(FILE *s) {
  File *f = file(s);
  return !f || f->pos == f->size;
}
int __wrap_ferror(FILE *s) { return file(s) ? 0 : 1; }
int __wrap_fflush(FILE *s) { return 0; }
int __wrap_fgetc(FILE *s) {
  unsigned char ch;
  return __wrap_fread(&ch, 1, 1, s) == 1 ? ch : EOF;
}
int __wrap_fputc(int ch, FILE *s) {
  unsigned char c = ch;
  return __wrap_fwrite(&c, 1, 1, s) == 1 ? c : EOF;
}
int __wrap_fprintf(FILE *s, const char *fmt, ...) { return 0; }
char *__wrap_fgets(char *out, int n, FILE *s) {
  if (n <= 1)
    return NULL;
  int i = 0, ch;
  while (i < n - 1 && (ch = __wrap_fgetc(s)) != EOF) {
    out[i++] = ch;
    if (ch == '\n')
      break;
  }
  out[i] = 0;
  return i ? out : NULL;
}
int __wrap_remove(const char *p) {
  if (kind(p) == 3) {
    save_length = 0;
    return 0;
  }
  return -1;
}
int __wrap_rename(const char *a, const char *b) {
  if (kind(a) != 2 || kind(b) != 3)
    return -1;
  memcpy(save_data, temp_data, temp_length);
  save_length = temp_length;
  dirty = 1;
  return 0;
}
int __wrap_mkdir(const char *p, mode_t mode) { return -1; }
char *__wrap_getenv(const char *p) { return NULL; }
int system(const char *command) { return -1; }
int __wrap_access(const char *p, int mode) { return kind(p) == 1 ? 0 : -1; }
/* Any remaining libc/WASI request is denied or captured, never forwarded. */
__wasi_errno_t __wrap___wasi_fd_write(__wasi_fd_t fd,
                                      const __wasi_ciovec_t *iov, size_t n,
                                      __wasi_size_t *written) {
  size_t total = 0;
  for (size_t i = 0; i < n; i++) {
    size_t count = iov[i].buf_len;
    if (count > sizeof(diagnostics) - 1 - diagnostic_length)
      count = sizeof(diagnostics) - 1 - diagnostic_length;
    memcpy(diagnostics + diagnostic_length, iov[i].buf, count);
    diagnostic_length += count;
    total += iov[i].buf_len;
  }
  diagnostics[diagnostic_length] = 0;
  *written = total;
  return 0;
}
__wasi_errno_t __wrap___wasi_fd_close(__wasi_fd_t fd) {
  return __WASI_ERRNO_BADF;
}
__wasi_errno_t __wrap___wasi_fd_fdstat_get(__wasi_fd_t fd,
                                           __wasi_fdstat_t *stat) {
  return __WASI_ERRNO_BADF;
}
__wasi_errno_t __wrap___wasi_fd_seek(__wasi_fd_t fd, __wasi_filedelta_t delta,
                                     __wasi_whence_t whence,
                                     __wasi_filesize_t *out) {
  return __WASI_ERRNO_BADF;
}
void __wrap___wasi_proc_exit(__wasi_exitcode_t code) { __builtin_trap(); }
void *__wrap_memcpy(void *a, const void *b, size_t n) {
  __builtin_memcpy(a, b, n);
  return a;
}
void *__wrap_memmove(void *a, const void *b, size_t n) {
  __builtin_memmove(a, b, n);
  return a;
}
void *__wrap_memset(void *a, int b, size_t n) {
  __builtin_memset(a, b, n);
  return a;
}
void DG_Init(void) {}
void DG_DrawFrame(void) {}
void DG_SetWindowTitle(const char *title) {}
void DG_SleepMs(uint32_t ms) { ticks += ms ? ms : 1; }
uint32_t DG_GetTicksMs(void) { return ticks; }
int DG_GetKey(int *pressed, unsigned char *key) {
  if (qr == qw)
    return 0;
  unsigned v = keyqueue[qr++ % 64];
  *pressed = v >> 8;
  *key = v;
  return 1;
}
static void event(int key, int down) {
  if (qw - qr < 64)
    keyqueue[qw++ % 64] = (down << 8) | key;
}
/* Deterministic mono mixer: DMX sound lumps, no devices or external codecs. */
typedef struct {
  const uint8_t *data;
  uint32_t length, phase, rate;
  int volume, active;
} Channel;
static Channel channels[16];
static boolean snd_init(boolean prefix) { return true; }
static void snd_shutdown(void) {}
static int snd_lump(sfxinfo_t *s) {
  char name[9];
  snprintf(name, sizeof(name), "ds%.6s", s->name);
  return W_GetNumForName(name);
}
static void snd_update(void) {}
static void snd_params(int c, int vol, int sep) {
  if (c >= 0 && c < 16)
    channels[c].volume = vol;
}
static int snd_start(sfxinfo_t *s, int c, int vol, int sep) {
  if (c < 0 || c >= 16)
    return -1;
  int lump = snd_lump(s);
  int size = W_LumpLength(lump);
  const uint8_t *d = W_CacheLumpNum(lump, PU_STATIC);
  if (size < 8 || d[0] != 3 || d[1] != 0)
    return -1;
  uint32_t n = d[4] | d[5] << 8 | d[6] << 16 | (uint32_t)d[7] << 24;
  if (n > (uint32_t)size - 8)
    n = size - 8;
  channels[c] = (Channel){d + 8, n, 0, (uint32_t)(d[2] | d[3] << 8), vol, 1};
  return c;
}
static void snd_stop(int c) {
  if (c >= 0 && c < 16)
    channels[c].active = 0;
}
static boolean snd_playing(int c) {
  return c >= 0 && c < 16 && channels[c].active;
}
static void snd_cache(sfxinfo_t *s, int n) {}
static snddevice_t devices[] = {SNDDEVICE_SB};
sound_module_t DG_sound_module = {
    devices,    1,         snd_init, snd_shutdown, snd_lump, snd_update,
    snd_params, snd_start, snd_stop, snd_playing,  snd_cache};
music_module_t DG_music_module = {0};
int use_libsamplerate = 0;
float libsamplerate_scale = 1;
void I_InitTimidityConfig(void) {}
static void mix(void) {
  for (int i = 0; i < 800; i++) {
    int sum = 0;
    for (int c = 0; c < 16; c++) {
      Channel *p = &channels[c];
      if (!p->active)
        continue;
      uint32_t pos = p->phase / 48000;
      if (pos >= p->length) {
        p->active = 0;
        continue;
      }
      sum += ((int)p->data[pos] - 128) * p->volume * 2;
      p->phase += p->rate;
    }
    if (sum > 32767)
      sum = 32767;
    if (sum < -32768)
      sum = -32768;
    pcm[i] = sum;
  }
}
EXPORT("allocate") void *allocate(uint32_t n) {
  return n <= 64 * 1024 * 1024 ? malloc(n) : NULL;
}
EXPORT("initialize")
int initialize(uint8_t *data, uint32_t length, uint32_t saved) {
  wad = data;
  wad_length = length;
  save_length = saved <= SAVE_MAX ? saved : 0;
  static char *args[] = {"gamepkg-doom",
                         "-iwad",
                         "doom.wad",
                         "-config",
                         "disabled.cfg",
                         "-extraconfig",
                         "disabled-extra.cfg",
                         "-savedir",
                         "gamepkg",
                         "-mb",
                         "16",
                         "-nomusic",
                         "-loadgame",
                         "0"};
  doomgeneric_Create(save_length ? 14 : 12, args);
  return 0;
}
EXPORT("step") int step(uint32_t input) {
  uint32_t now = (input & 0x000f00ff) | ((input >> 8) & 0xff) |
                 ((input >> 4) & 0x000f0000);
  buttons = now;
  const uint32_t masks[] = {1,  2,   4,       8,       16,      32,
                            64, 128, 1 << 16, 1 << 17, 1 << 18, 1 << 19};
  const uint8_t keys[] = {
      KEY_LEFTARROW, KEY_RIGHTARROW, KEY_UPARROW, KEY_DOWNARROW,
      KEY_FIRE,      KEY_RSHIFT,     KEY_ENTER,   KEY_ESCAPE,
      KEY_USE,       KEY_TAB,        ',',         '.'};
  for (int i = 0; i < 12; i++)
    if ((now ^ previous) & masks[i])
      event(keys[i], !!(now & masks[i]));
  if ((now & ((1 << 18) | (1 << 19) | 64)) == ((1 << 18) | (1 << 19) | 64) &&
      !(previous & 64) && gamestate == GS_LEVEL)
    G_SaveGame(0, "GAMEPKG");
  previous = now;
  accumulator += 35;
  if (accumulator >= 60) {
    accumulator -= 60;
    ticks += 28;
    doomgeneric_Tick();
  }
  mix();
  return 0;
}
EXPORT("pixels") void *pixels(void) { return DG_ScreenBuffer; }
EXPORT("audio") void *audio(void) { return pcm; }
EXPORT("save") void *savedata(void) { return save_data; }
EXPORT("save_length") uint32_t savedlength(void) { return save_length; }
EXPORT("save_dirty") int savedirty(void) {
  int d = dirty;
  dirty = 0;
  return d;
}
EXPORT("status") uint32_t status(void) {
  return gamestate | (demoplayback ? 256 : 0) | ((uint32_t)gamemap << 16);
}
EXPORT("position") uint32_t position(void) {
  return players[consoleplayer].mo
             ? (uint16_t)(players[consoleplayer].mo->x >> 16) |
                   ((uint32_t)(uint16_t)(players[consoleplayer].mo->y >> 16)
                    << 16)
             : 0;
}
EXPORT("controller") uint32_t controller(void) { return buttons; }
EXPORT("diagnostics") void *logs(void) { return diagnostics; }
/* Restart the first level through the engine's deferred game action. */
EXPORT("reset") void reset_engine(void) {
  G_DeferedInitNew(sk_medium, 1, 1);
  previous = 0;
  qr = qw = 0;
}
