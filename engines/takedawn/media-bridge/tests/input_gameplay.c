/* Asset-free regression against the complete checksum-staged controls.c.
 * Synthetic RAM represents controller assignments, not original game assets.
 */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include GAMEPKG_CONTROLS_SOURCE
#ifdef GAMEPKG_EXPECT_POINTER_BITS
_Static_assert(sizeof(void *) * 8 == GAMEPKG_EXPECT_POINTER_BITS,
               "The input regression must use the requested pointer width");
#endif

ptrdiff_t g_xbox_mem_offset;
static unsigned checks;
#define CHECK(value) do { assert(value); checks++; } while (0)

static void binding(unsigned player, unsigned record, unsigned port)
{
    uint32_t device = 0x12000u + record * 0x200u;
    M8(GAMEPKG_CTL_ASSIGNMENTS + player) = (uint8_t)record;
    M32(GAMEPKG_CTL_CONTROLLERS + record * GAMEPKG_CTL_CONTROLLER_SIZE + 4) = device;
    M32(device) = GAMEPKG_CTL_VT_DEVICE;
    M32(device + 0x118u) = CTL_PADS;
    M32(device + 0x11Cu) = port;
}

static void playing(unsigned players)
{
    M32(CTL_MODULE) = 0x10000u;
    M32(0x10000u) = players == 1 ? CTL_VT_GAME : GAMEPKG_CTL_VT_MULTI;
    M32(CTL_PAUSED_BY) = 0xFFFFFFFFu;
    M32(CTL_MODULE_STEP) = 5;
    M8(CTL_NO_PAUSE) = 0;
    for (unsigned player = 0; player < 2; player++) {
        M8(CTL_P1 + player * CTL_P_SIZE + CTL_P_RESULTS) = 0;
        M8(CTL_P1 + player * CTL_P_SIZE + CTL_P_INTRO) = 0;
        M32(CTL_PHASE + player * GAMEPKG_CTL_CAR_SIZE) = 2;
        M32(CTL_COUNTDOWN + player * GAMEPKG_CTL_CAR_SIZE) = 0x40900000u;
        binding(player, player, player + 2);
    }
}

static void mappings(void)
{
    /* Every distinct pair of physical ports, in both assignment orders.
     * Record indices deliberately do not equal ports or player numbers. */
    for (unsigned a = 0; a < 4; a++) {
        playing(1); binding(0, 3, a);
        for (unsigned port = 0; port < 4; port++)
            CHECK(gamepkg_ctl_port_driving(port) == (port == a));
        for (unsigned b = 0; b < 4; b++) if (a != b) {
            playing(2); binding(0, 3, a); binding(1, 2, b);
            for (unsigned port = 0; port < 4; port++)
                CHECK(gamepkg_ctl_port_driving(port) == (port == a || port == b));
            M8(CTL_P1 + CTL_P_RESULTS) = 1;
            M32(CTL_PHASE) = 3;
            M8(CTL_NO_PAUSE) = 1;
            CHECK(!gamepkg_ctl_port_driving(a));
            CHECK(gamepkg_ctl_port_driving(b));
            M8(CTL_P1 + CTL_P_RESULTS) = 0; M32(CTL_PHASE) = 2;
            M8(CTL_P1 + CTL_P_SIZE + CTL_P_RESULTS) = 1;
            M32(CTL_PHASE + GAMEPKG_CTL_CAR_SIZE) = 3;
            CHECK(gamepkg_ctl_port_driving(a));
            CHECK(!gamepkg_ctl_port_driving(b));
        }
    }
}

static void transitions(void)
{
    for (unsigned players = 1; players <= 2; players++) {
        playing(players);
        for (unsigned paused = 0; paused < 4; paused++) {
            M32(CTL_PAUSED_BY) = paused;
            CHECK(!gamepkg_ctl_port_driving(2));
            CHECK(!gamepkg_ctl_port_driving(3));
        }
        M32(CTL_PAUSED_BY) = 0xFFFFFFFFu;
        M32(CTL_MODULE_STEP) = 6;
        CHECK(!gamepkg_ctl_port_driving(2)); CHECK(!gamepkg_ctl_port_driving(3));
        M32(CTL_MODULE_STEP) = 5;
        for (unsigned player = 0; player < players; player++) {
            unsigned port = player + 2;
            uint32_t state = CTL_P1 + player * CTL_P_SIZE;
            uint32_t phase = CTL_PHASE + player * GAMEPKG_CTL_CAR_SIZE;
            uint32_t cd = CTL_COUNTDOWN + player * GAMEPKG_CTL_CAR_SIZE;
            M8(state + CTL_P_INTRO) = 1;
            CHECK(!gamepkg_ctl_port_driving(port)); M8(state + CTL_P_INTRO) = 0;
            M8(state + CTL_P_RESULTS) = 1;
            CHECK(!gamepkg_ctl_port_driving(port)); M8(state + CTL_P_RESULTS) = 0;
            for (unsigned p = 0; p < 2; p++) {
                M32(phase) = p;
                M32(cd) = 0; CHECK(!gamepkg_ctl_port_driving(port));
                M32(cd) = 0xBF800000u; CHECK(!gamepkg_ctl_port_driving(port));
                M32(cd) = 0x40900000u; CHECK(gamepkg_ctl_port_driving(port));
            }
            M32(phase) = 3; CHECK(!gamepkg_ctl_port_driving(port));
            M32(phase) = 4; CHECK(!gamepkg_ctl_port_driving(port));
            M32(phase) = 2; CHECK(gamepkg_ctl_port_driving(port));
        }
        M8(CTL_NO_PAUSE) = 1;
        CHECK(gamepkg_ctl_port_driving(2) == (players == 2));
    }
}

static void malformed(void)
{
    static const uint32_t bad[] = {0, 0xFFFFu, 0x10001u, 0x04000000u, 0xFFFFFFFCu};
    for (unsigned i = 0; i < sizeof bad / sizeof bad[0]; i++) {
        playing(2); M32(CTL_MODULE) = bad[i];
        CHECK(!gamepkg_ctl_port_driving(2));
        playing(2); M32(GAMEPKG_CTL_CONTROLLERS + 4) = bad[i];
        CHECK(!gamepkg_ctl_port_driving(2)); CHECK(gamepkg_ctl_port_driving(3));
    }
    playing(2); M32(0x10000u) = 0x003A9E7Cu;
    CHECK(!gamepkg_ctl_port_driving(2)); CHECK(!gamepkg_ctl_port_driving(3));
    playing(2); M8(GAMEPKG_CTL_ASSIGNMENTS) = 0xFF;
    CHECK(!gamepkg_ctl_port_driving(2)); CHECK(gamepkg_ctl_port_driving(3));
    playing(2); M32(0x12000u) = 0;
    CHECK(!gamepkg_ctl_port_driving(2));
    playing(2); M32(0x12118u) = CTL_PADS + 0x20u;
    CHECK(!gamepkg_ctl_port_driving(2));
    playing(2); M32(0x1211Cu) = 4;
    CHECK(!gamepkg_ctl_port_driving(2));
    playing(2); binding(1, 1, 2);
    CHECK(!gamepkg_ctl_port_driving(2)); CHECK(!gamepkg_ctl_port_driving(3));
    CHECK(!gamepkg_ctl_port_driving(4)); CHECK(!gamepkg_ctl_port_driving(UINT32_MAX));
}

static void held_button_handover(void)
{
    for (int nintendo = 0; nintendo <= 1; nintendo++) {
        bo3_ctl_port ports[2] = {{0}, {0}};
        playing(2); binding(0, 3, 3); binding(1, 2, 2);
        for (unsigned player = 0; player < 2; player++) {
            XBOX_GAMEPAD pad = {0}; pad.bAnalogButtons[XBOX_BUTTON_A] = 255;
            if (nintendo) bo3_ctl_mirror(&pad);
            bo3_ctl_apply(&ports[player], &pad,
                          gamepkg_ctl_port_driving(player ? 2 : 3), nintendo);
            CHECK(pad.bAnalogButtons[XBOX_BUTTON_RTRIGGER] == 255);
            CHECK(pad.bAnalogButtons[XBOX_BUTTON_A] == 0);
        }
        M8(CTL_P1 + CTL_P_RESULTS) = 1; M32(CTL_PHASE) = 3;
        for (unsigned player = 0; player < 2; player++) {
            XBOX_GAMEPAD pad = {0}; pad.bAnalogButtons[XBOX_BUTTON_A] = 255;
            if (nintendo) bo3_ctl_mirror(&pad);
            bo3_ctl_apply(&ports[player], &pad,
                          gamepkg_ctl_port_driving(player ? 2 : 3), nintendo);
            CHECK(pad.bAnalogButtons[XBOX_BUTTON_RTRIGGER] == (player ? 255 : 0));
            CHECK(pad.bAnalogButtons[XBOX_BUTTON_A] == 0);
            CHECK(pad.bAnalogButtons[XBOX_BUTTON_B] == 0);
        }
    }
}

int main(void)
{
    void *ram = calloc(1, 0x04000000u); assert(ram);
    g_xbox_mem_offset = (ptrdiff_t)ram;
    mappings(); transitions(); malformed(); held_button_handover();
    free(ram);
    printf("PASS: %u original input-gate/assignment/transition checks (%zu-bit pointers)\n",
           checks, sizeof(void *) * 8);
    return 0;
}
