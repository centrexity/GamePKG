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
static uint8_t *wad;
static uint32_t wad_length;
static char diagnostics[4096];
static uint32_t diagnostic_length;
static uint32_t ticks = 1, previous, buttons, accumulator;
static int quit_requested;
static unsigned quit_countdown;
static uint8_t pointer_buttons;
void gamepkg_request_quit(void) { if(!quit_requested) quit_requested=2; }

#include "audio.h"
#include "storage.h"
#include "m_config.h"
#include "i_system.h"
#include "d_event.h"
#include "m_controls.h"
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
#include "input.h"
#include "endoom.h"
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
    channel_params(&channels[c], vol, sep);
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
  channels[c] = (Channel){.data = d + 8, .length = n,
                          .rate = (uint32_t)(d[2] | d[3] << 8), .active = 1};
  channel_params(&channels[c], vol, sep);
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
void gamepkg_music_render(int16_t *samples, unsigned frames);
int use_libsamplerate = 0;
float libsamplerate_scale = 1;
void I_InitTimidityConfig(void) {}
EXPORT("allocate") void *allocate(uint32_t n) {
  return n <= 64 * 1024 * 1024 ? malloc(n) : NULL;
}
EXPORT("initialize")
int initialize(uint8_t *data, uint32_t length, uint32_t saved) {
  wad = data;
  wad_length = length;
  if (restore_save(saved) != 0) return -1;
  static char selected_slot[2];
  selected_slot[0]=(char)('0'+(last_slot<0?0:last_slot));
  static char *args[] = {"gamepkg-doom",
                         "-iwad",
                         "doom.wad",
                         "-config",
                         "gamepkg.cfg",
                         "-extraconfig",
                         "gamepkg-extra.cfg",
                         "-savedir",
                         "gamepkg",
                         "-mb",
                         "16",
                         "-loadgame",
                         selected_slot};
  doomgeneric_Create(last_slot >= 0 ? 13 : 11, args);
  /* Migrate only the old adapter's four unserializable proxy defaults. */
  if(!key_fire && !key_use && !key_strafeleft && !key_straferight) {
    key_fire=KEY_RCTRL; key_use=' '; key_strafeleft=','; key_straferight='.';
  }
  return 0;
}
EXPORT("step") int step(uint32_t input) {
  if(quit_requested==2) { memset(pcm,0,sizeof(pcm)); return 0; }
  uint32_t now = (input & 0x000f00ff) | ((input >> 8) & 0xff) |
                 ((input >> 4) & 0x000f0000);
  if(quit_requested==1) {
    if(now&~ending_mask) quit_requested=2;
    ending_mask=now; render_ending(); memset(pcm,0,sizeof(pcm)); return 0;
  }
  buttons = now;
  const uint32_t masks[] = {1,  2,   4,       8,       16,      32,
                            64, 128, 1 << 16, 1 << 17, 1 << 18, 1 << 19};
  const uint8_t keys[] = {
      key_left, key_right, key_up, key_down,
      key_fire,      key_speed,     KEY_ENTER,   KEY_ESCAPE,
      key_use,       KEY_TAB,        key_strafeleft, key_straferight};
  action_state(keys,masks,12,now);
  if ((now & ((1 << 18) | (1 << 19) | 64)) == ((1 << 18) | (1 << 19) | 64) &&
      !(previous & 64) && gamestate == GS_LEVEL)
    G_SaveGame(0, "GAMEPKG");
  previous = now;
  accumulator += 35;
  if (!quit_countdown && accumulator >= 60) {
    accumulator -= 60;
    ticks += 28;
    input_dispatched=0;
    doomgeneric_Tick();
  }
  M_SaveDefaults();
  if (storage_error) I_Error("DOOM virtual save or configuration storage failed");
  if(quit_countdown) memcpy(DG_ScreenBuffer,quit_frame,sizeof(quit_frame));
  mix();
  int16_t music[1600];
  gamepkg_music_render(music, 800);
  for (int i = 0; i < 1600; i++) pcm[i] = clip_sample((int)pcm[i] + music[i]);
  if(quit_countdown && --quit_countdown==0) {
    I_Quit(); ending_mask=now;
    if(quit_requested==1) render_ending();
  }
  return 0;
}
EXPORT("pixels") void *pixels(void) { return DG_ScreenBuffer; }
EXPORT("audio") void *audio(void) { return pcm; }
EXPORT("audio_channels") uint32_t audio_channels(void) { return 2; }
EXPORT("save") void *savedata(void) { return save_data; }
EXPORT("save_capacity") uint32_t savecapacity(void) { return BUNDLE_MAX; }
EXPORT("save_length") uint32_t savedlength(void) { return save_length; }
EXPORT("save_dirty") int savedirty(void) {
  int d = dirty;
  if (dirty) snapshot_save();
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
EXPORT("controller") uint32_t controller(void) { return buttons | (players[consoleplayer].attackdown?1u<<24:0) | (paused?1u<<26:0) | (quit_requested==1?1u<<25:0) | ((uint32_t)players[consoleplayer].readyweapon<<27); }
EXPORT("quit_requested") int quitrequested(void) { return quit_requested==2; }
EXPORT("diagnostics") void *logs(void) { return diagnostics; }
/* Restart the first level through the engine's deferred game action. */
EXPORT("reset") void reset_engine(void) {
  G_DeferedInitNew(sk_medium, 1, 1);
  previous = 0;
  engine_input(3,0,0);
  for(unsigned key=1;key<256;key++) set_key(action_keys,key,0,0);
}
