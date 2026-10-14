// Host-owned Zelda3 Wasm adapter. MIT upstream notices remain in upstream/LICENSE.txt.
#include "src/zelda_rtl.h"
#include "src/variables.h"
#include "src/assets.h"
#include "src/config.h"
#include "src/spc_player.h"
#include "src/audio.h"
#include "src/util.h"
#include "snes/dsp.h"
#include "snes/ppu.h"
#include "snes/snes_regs.h"
Config g_config;
const uint8 *g_asset_ptrs[kNumberOfAssets];
uint32 g_asset_sizes[kNumberOfAssets];
static uint8 ports[4];
static uint8 framebuffer[512 * 224 * 4];
static int16 samples[800];
static bool initialized;
static bool save_dirty;

void ZeldaApuLock(void) {} // One Wasm instance, one tick thread.
void ZeldaApuUnlock(void) {}
void zelda_apu_write(uint32_t address, uint8 value) { ports[address & 3] = value; }
uint8 zelda_apu_read(uint32_t address) { return g_zenv.player->port_to_snes[address & 3]; }
uint8 zelda_read_apui00(void) { return g_ram[kRam_APUI00]; }
void ZeldaPushApuState(void) { memcpy(g_zenv.player->input_ports, ports, 4); }
void ZeldaDiscardUnusedAudioFrames(void) {}
bool ZeldaIsMusicPlaying(void) { return g_zenv.player->port_to_snes[0] != 0; }
bool ZeldaIsPlayingMusicTrack(uint8 track) { return music_unk1 == track; }
bool ZeldaIsPlayingMusicTrackWithBug(uint8 track) { return track == (enhanced_features0 & kFeatures0_MiscBugFixes ? music_unk1 : last_music_control); }
void ZeldaPlayMsuAudioTrack(uint8 track) { zelda_apu_write(APUI00, track); }
void ZeldaEnableMsu(uint8 enable) { (void)enable; }
uint8 ZeldaGetEntranceMusicTrack(int i) { return kEntranceData_musicTrack[i]; }
void LoadSongBank(const uint8 *data) { SpcPlayer_Upload(g_zenv.player, data); }
void ZeldaRestoreMusicAfterLoad_Locked(bool reset) {
  if (reset) SpcPlayer_Initialize(g_zenv.player);
  else SpcPlayer_CopyVariablesFromRam(g_zenv.player);
}
void ZeldaSaveMusicStateToRam_Locked(void) { SpcPlayer_CopyVariablesToRam(g_zenv.player); }
void ZeldaRenderAudio(int16 *buffer, int count, int channels) {
  SpcPlayer_GenerateSamples(g_zenv.player);
  dsp_getSamples(g_zenv.player->dsp, buffer, count, channels);
}
void ZeldaReadSram(void) {} // Scoped save bytes supplied by the host, never a filename.
void ZeldaWriteSram(void) { save_dirty = true; }
MemBlk FindInAssetArray(int asset, int index) {
  if (asset < 0 || asset >= kNumberOfAssets) return (MemBlk){0,0};
  return FindIndexInMemblk((MemBlk){g_asset_ptrs[asset],g_asset_sizes[asset]}, index);
}
void Die(const char *message) { (void)message; __builtin_trap(); }
void abort(void) { __builtin_trap(); }
void __assert_fail(const char *expr, const char *file, int line, const char *function) {
  (void)expr; (void)file; (void)line; (void)function; __builtin_trap();
}
static uint32 read32(const uint8 *p) { return p[0] | p[1]<<8 | p[2]<<16 | (uint32)p[3]<<24; }
__attribute__((export_name("allocate")))
uint8 *allocate(uint32 size) { return size <= 8388608 ? malloc(size) : NULL; }
__attribute__((export_name("initialize")))
int initialize(uint8 *data, uint32 length) {
  static const uint8 signature[48] = {kAssets_Sig};
  if (initialized || length < 748 || memcmp(data, signature, 48) || read32(data+80) != 165) return -1;
  uint32 names=read32(data+84);
  if (names>length-748) return -1;
  uint32 offset=748+names;
  for (uint32 i=0;i<165;i++) {
    uint32 size=read32(data+88+i*4);
    offset=(offset+3)&~3u;
    if (offset>length || size>length-offset) return -1;
    g_asset_ptrs[i]=data+offset;g_asset_sizes[i]=size;offset+=size;
  }
  ZeldaInitialize(); g_zenv.ppu->extraLeftRight=0; ZeldaSetLanguage(NULL); initialized=true; return 0;
}
__attribute__((export_name("step")))
int step(uint32 buttons) {
  if (!initialized) return -1;
  // Preserve the upstream scheduling without its unbounded desktop replay log.
  g_ram[kRam_APUI00]=ZeldaIsMusicPlaying();
  if (animated_tile_data_src != 0) {
    g_ram[kRam_BugsFixed] = kBugFix_Latest;
    enhanced_features0 = g_wanted_zelda_features;
  }
  int run_what;
  if (g_ram[kRam_BugsFixed] < kBugFix_PolyRenderer) {
    run_what = (is_nmi_thread_active && thread_other_stack != 0x1f31) ? 2 : 1;
  } else {
    int counter = g_ram[kRam_CrystalRotateCounter] + virq_trigger;
    g_ram[kRam_CrystalRotateCounter] = counter;
    run_what = (is_nmi_thread_active && (counter >> 8)) ? 3 : 1;
  }
  ZeldaRunFrameInternal((uint16)buttons,run_what);
  ZeldaPushApuState(); ZeldaDrawPpuFrame(framebuffer,256*4,kPpuRenderFlags_NewRenderer);
  ZeldaRenderAudio(samples,800,1); return 0;
}
__attribute__((export_name("pixels"))) uint8 *pixels(void) { return framebuffer; }
__attribute__((export_name("audio"))) int16 *audio(void) { return samples; }
__attribute__((export_name("sram"))) uint8 *sram(void) { return initialized ? g_zenv.sram : NULL; }
__attribute__((export_name("save_dirty"))) int save_changed(void) { int result=save_dirty;save_dirty=false;return result; }
// libc diagnostic paths cannot reach ambient WASI descriptors.
#include <wasi/api.h>
__wasi_errno_t __wrap___wasi_fd_close(__wasi_fd_t fd) { (void)fd; return __WASI_ERRNO_BADF; }
__wasi_errno_t __wrap___wasi_fd_fdstat_get(__wasi_fd_t fd, __wasi_fdstat_t *stat) { (void)fd;(void)stat;return __WASI_ERRNO_BADF; }
__wasi_errno_t __wrap___wasi_fd_seek(__wasi_fd_t fd, __wasi_filedelta_t delta, __wasi_whence_t whence, __wasi_filesize_t *offset) {
  (void)fd;(void)delta;(void)whence;(void)offset;return __WASI_ERRNO_BADF;
}
__wasi_errno_t __wrap___wasi_fd_write(__wasi_fd_t fd, const __wasi_ciovec_t *iov, size_t count, __wasi_size_t *written) {
  (void)fd;(void)iov;(void)count;if(written)*written=0;return __WASI_ERRNO_BADF;
}
__attribute__((export_name("reset"))) void reset_engine(void) { if(initialized) ZeldaReset(true); }
// Bounded Wasm bulk-memory operations avoid byte-loop fuel spikes during loading.
void *__wrap_memcpy(void *target,const void *source,size_t length) { __builtin_memcpy(target,source,length);return target; }
void *__wrap_memmove(void *target,const void *source,size_t length) { __builtin_memmove(target,source,length);return target; }
void *__wrap_memset(void *target,int value,size_t length) { __builtin_memset(target,value,length);return target; }
// Read-only diagnostics for verifying controller response and save/reload tests.
__attribute__((export_name("status"))) uint32 engine_status(void) { return main_module_index | submodule_index<<8 | subsubmodule_index<<16 | (uint32)text_render_state<<24; }
__attribute__((export_name("position"))) uint32 engine_position(void) { return link_x_coord | (uint32)link_y_coord<<16; }

__attribute__((export_name("controller"))) uint32 engine_controller(void) { return joypad1H_last | joypad1L_last<<8 | (uint32)filtered_joypad_H<<16 | (uint32)filtered_joypad_L<<24; }
