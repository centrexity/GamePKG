/* Reviewed native input policy for the pinned game's own player state.
 *
 * The title assigns controllers; USB slot order is not player ownership.
 * sub_0001A000 indexes the signed-byte assignment table at 0x4AED45, then
 * selects a 0x188-byte controller record at 0x4AE724. sub_00019A10 reads its
 * device pointer at +4. The original device update, sub_001B5180, selects
 * the XInput report using device +0x118 (report array) and +0x11C (port).
 * Read those same bindings, without changing the title's input or state.
 *
 * M8/M32 and CTL_* are supplied by the checksum-pinned controls.c. These
 * functions run on its existing pad-read hook; no game function is called.
 */
#ifndef GAMEPKG_INPUT_GAMEPLAY_POLICY_H
#define GAMEPKG_INPUT_GAMEPLAY_POLICY_H

#define GAMEPKG_CTL_VT_MULTI 0x003A939Cu
#define GAMEPKG_CTL_VT_DEVICE 0x003A9B04u
#define GAMEPKG_CTL_ASSIGNMENTS 0x004AED45u
#define GAMEPKG_CTL_CONTROLLERS 0x004AE724u
#define GAMEPKG_CTL_CONTROLLER_SIZE 0x188u
#define GAMEPKG_CTL_CAR_SIZE 0x27E0u
#define GAMEPKG_CTL_SHARED_REPLAY 0x00752D32u

static int gamepkg_ctl_words(uint32_t address, uint32_t bytes)
{
    return !(address & 3u) && address >= 0x10000u && bytes <= 0x04000000u
        && address <= 0x04000000u - bytes;
}

static unsigned gamepkg_ctl_players(void)
{
    uint32_t mod = M32(CTL_MODULE), vt;
    if (!gamepkg_ctl_words(mod, 4)) return 0;
    vt = M32(mod);
    return vt == CTL_VT_GAME ? 1u : vt == GAMEPKG_CTL_VT_MULTI ? 2u : 0u;
}

static int gamepkg_ctl_player_port(unsigned player)
{
    uint32_t index, device, port;
    if (player >= gamepkg_ctl_players()) return -1;
    index = M8(GAMEPKG_CTL_ASSIGNMENTS + player);
    /* Includes the original signed-byte unassigned value, -1. */
    if (index >= 4) return -1;
    device = M32(GAMEPKG_CTL_CONTROLLERS
                 + index * GAMEPKG_CTL_CONTROLLER_SIZE + 4);
    if (!gamepkg_ctl_words(device, 0x120u)
        || M32(device) != GAMEPKG_CTL_VT_DEVICE
        || M32(device + 0x118u) != CTL_PADS) return -1;
    port = M32(device + 0x11Cu);
    return port < 4 ? (int)port : -1;
}

static int gamepkg_ctl_player_driving(unsigned player)
{
    unsigned players = gamepkg_ctl_players();
    uint32_t state, phase, countdown;
    if (player >= players || M32(CTL_PAUSED_BY) != 0xFFFFFFFFu
        || M32(CTL_MODULE_STEP) == 6) return 0;
    /* Preserve the original single-player transition guard. In split screen
     * this flag controls whether the shared stage accepts Pause; it cannot
     * classify each player's input. One player's results must not turn the
     * other player's accelerator back into a menu button. */
    if (players == 1 && M8(CTL_NO_PAUSE)) return 0;
    /* Shared results can run a looping replay behind their menu. Its cars
     * keep returning to phase 2 without either per-player results byte set.
     * The original stage replay flag (set by sub_0001A8B0), together with
     * the no-pause flag, distinguishes this from one player finishing while
     * the other still drives. Keep confirmation and replay-menu buttons in
     * their menu layout, regardless of the background cars' phase. */
    if (players == 2 && M8(CTL_NO_PAUSE) && M8(GAMEPKG_CTL_SHARED_REPLAY)) return 0;
    state = CTL_P1 + player * CTL_P_SIZE;
    if (M8(state + CTL_P_INTRO) || M8(state + CTL_P_RESULTS)) return 0;
    phase = M32(CTL_PHASE + player * GAMEPKG_CTL_CAR_SIZE);
    countdown = M32(CTL_COUNTDOWN + player * GAMEPKG_CTL_CAR_SIZE);
    return phase == 2 || (phase < 2 && countdown != 0xBF800000u && countdown != 0);
}

static int gamepkg_ctl_port_driving(unsigned port)
{
    int owner = -1;
    if (port >= 4) return 0;
    for (unsigned player = 0; player < gamepkg_ctl_players(); player++) {
        if (gamepkg_ctl_player_port(player) != (int)port) continue;
        /* Ambiguous or corrupt assignments must not consume a menu press. */
        if (owner >= 0) return 0;
        owner = (int)player;
    }
    return owner >= 0 && gamepkg_ctl_player_driving((unsigned)owner);
}

static int ctl_driving(void)
{
    return gamepkg_ctl_player_driving(0);
}
#endif
