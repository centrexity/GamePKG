/* GamePKG host-owned, single-threaded software OPL backend.
 * GPL-2.0-or-later. No hardware, clock, filesystem, device or thread access.
 */
#include <string.h>
#include <limits.h>
#include "music_compat.h"
#include "i_system.h"
#include "upstream/opl.h"
#include "upstream/opl3.h"
#define CALLBACK_LIMIT 128
static opl3_chip chip;
static struct callback { uint64_t due; opl_callback_t function; void *data; } callbacks[CALLBACK_LIMIT];
static unsigned count;
static uint64_t time_us;
static unsigned fraction;
static int paused;
static int initialized;
static unsigned sample_rate = 48000;
opl_init_result_t OPL_Init(unsigned port) {
    (void)port;
    if (sample_rate != 48000) return OPL_INIT_NONE;
    OPL3_Reset(&chip, sample_rate);
    count = 0; time_us = 0; fraction = 0; paused = 0;
    initialized = 1;
    return OPL_INIT_OPL3;
}
void OPL_Shutdown(void) { count = 0; initialized = 0; }
void OPL_SetSampleRate(unsigned rate) { sample_rate = rate; }
void OPL_WriteRegister(int reg, int value) {
    if (reg >= 0 && reg < 512) OPL3_WriteRegBuffered(&chip, reg, value);
}
void OPL_InitRegisters(int opl3) {
    for (int bank = 0; bank <= !!opl3; bank++) {
        if (bank) OPL_WriteRegister(OPL_REG_NEW, 1);
        for (int r = OPL_REGS_LEVEL; r <= OPL_REGS_LEVEL + OPL_NUM_OPERATORS; r++)
            OPL_WriteRegister(r | (bank << 8), 0x3f);
        for (int r = OPL_REGS_ATTACK; r <= OPL_REGS_WAVEFORM + OPL_NUM_OPERATORS; r++)
            OPL_WriteRegister(r | (bank << 8), 0);
        for (int r = 1; r < OPL_REGS_LEVEL; r++) OPL_WriteRegister(r | (bank << 8), 0);
    }
    OPL_WriteRegister(OPL_REG_TIMER_CTRL, 0x60);
    OPL_WriteRegister(OPL_REG_TIMER_CTRL, 0x80);
    OPL_WriteRegister(OPL_REG_WAVEFORM_ENABLE, 0x20);
    OPL_WriteRegister(OPL_REG_FM_MODE, 0x40);
    if (opl3) OPL_WriteRegister(OPL_REG_NEW, 1);
}
void OPL_SetCallback(uint64_t delay, opl_callback_t function, void *data) {
    if (count == CALLBACK_LIMIT || !function || delay > UINT64_MAX - time_us)
        I_Error("invalid OPL callback or callback budget exceeded");
    uint64_t due = time_us + delay;
    unsigned index = count++;
    while (index && callbacks[index - 1].due > due) {
        callbacks[index] = callbacks[index - 1]; index--;
    }
    callbacks[index] = (struct callback){due, function, data};
}
void OPL_AdjustCallbacks(float factor) {
    if (!(factor > 0)) I_Error("invalid OPL tempo");
    for (unsigned i = 0; i < count; i++) {
        uint64_t offset = callbacks[i].due > time_us ? callbacks[i].due - time_us : 0;
        double adjusted = (double)offset / factor;
        if (adjusted >= (double)(UINT64_MAX - time_us)) I_Error("OPL tempo overflow");
        callbacks[i].due = time_us + (uint64_t)adjusted;
    }
}
void OPL_ClearCallbacks(void) { count = 0; }
/* All callbacks execute synchronously during render on the engine thread. */
void OPL_Lock(void) {}
void OPL_Unlock(void) {}
void OPL_SetPaused(int value) { paused = !!value; }
void gamepkg_music_render(int16_t *samples, unsigned frames) {
    if (frames > 800) I_Error("OPL frame budget exceeded");
    if (!initialized) {
        memset(samples, 0, frames * 2 * sizeof(*samples));
        return;
    }
    for (unsigned frame = 0; frame < frames; frame++) {
        if (!paused) {
            unsigned dispatched = 0;
            while (count && callbacks[0].due <= time_us) {
                if (++dispatched > 4096) I_Error("OPL event budget exceeded");
                struct callback next = callbacks[0];
                memmove(callbacks, callbacks + 1, --count * sizeof(callbacks[0]));
                next.function(next.data);
            }
            fraction += 1000000;
            time_us += fraction / 48000;
            fraction %= 48000;
        }
        OPL3_GenerateResampled(&chip, samples + frame * 2);
    }
}

#ifdef GAMEPKG_OPL_TEST
/* Maintainer-only visibility; absent from game engine builds. */
unsigned gamepkg_opl_pending_callbacks(void) { return count; }
#endif
