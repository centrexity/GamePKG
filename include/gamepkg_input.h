#ifndef GAMEPKG_INPUT_H
#define GAMEPKG_INPUT_H
#include <stdint.h>
#define GAMEPKG_INPUT_ABI 1u
#define GAMEPKG_INPUT_MAX_DEVICES 16u
#define GAMEPKG_INPUT_MAX_PLAYERS 4u
#define GAMEPKG_INPUT_MAX_EVENTS 2048u
#define GAMEPKG_INPUT_MAX_TOUCHES 16u
#define GAMEPKG_INPUT_KEYBOARD 1u
#define GAMEPKG_INPUT_MOUSE 2u
#define GAMEPKG_INPUT_GAMEPAD 3u
#define GAMEPKG_INPUT_TOUCH 4u
#define GAMEPKG_INPUT_RESET 0u
#define GAMEPKG_INPUT_KEY_DOWN 1u
#define GAMEPKG_INPUT_KEY_UP 2u
#define GAMEPKG_INPUT_POINTER 3u
#define GAMEPKG_INPUT_RELATIVE 4u
#define GAMEPKG_INPUT_MOUSE_DOWN 5u
#define GAMEPKG_INPUT_MOUSE_UP 6u
#define GAMEPKG_INPUT_WHEEL 7u
#define GAMEPKG_INPUT_TOUCH_BEGIN 8u
#define GAMEPKG_INPUT_TOUCH_MOVE 9u
#define GAMEPKG_INPUT_TOUCH_END 10u
#define GAMEPKG_INPUT_FOCUS 11u
#define GAMEPKG_INPUT_CONNECTED 12u
#define GAMEPKG_INPUT_DISCONNECTED 13u
#define GAMEPKG_INPUT_PAD_DOWN 14u
#define GAMEPKG_INPUT_PAD_UP 15u
#define GAMEPKG_INPUT_PAD_AXIS 16u
#define GAMEPKG_INPUT_TEXT 17u
#define GAMEPKG_INPUT_PLAYER 18u
/* Button bits: D-pad up/down/left/right, Start, Back, left/right stick,
   left/right shoulder, Guide, Misc, South, East, West, North, touchpad,
   paddles 1..4. Axis event codes 0..5: LX, LY, RX, RY, LT, RT. Sticks
   use signed 16-bit values (Y up); triggers use the full unsigned range.
   No deadzone, smoothing or acceleration is applied by the service. */
#define GAMEPKG_INPUT_PAD_DPAD_UP (1u << 0)
#define GAMEPKG_INPUT_PAD_DPAD_DOWN (1u << 1)
#define GAMEPKG_INPUT_PAD_DPAD_LEFT (1u << 2)
#define GAMEPKG_INPUT_PAD_DPAD_RIGHT (1u << 3)
#define GAMEPKG_INPUT_PAD_START (1u << 4)
#define GAMEPKG_INPUT_PAD_BACK (1u << 5)
#define GAMEPKG_INPUT_PAD_LEFT_STICK (1u << 6)
#define GAMEPKG_INPUT_PAD_RIGHT_STICK (1u << 7)
#define GAMEPKG_INPUT_PAD_LEFT_SHOULDER (1u << 8)
#define GAMEPKG_INPUT_PAD_RIGHT_SHOULDER (1u << 9)
#define GAMEPKG_INPUT_PAD_GUIDE (1u << 10)
#define GAMEPKG_INPUT_PAD_MISC (1u << 11)
#define GAMEPKG_INPUT_PAD_SOUTH (1u << 12)
#define GAMEPKG_INPUT_PAD_EAST (1u << 13)
#define GAMEPKG_INPUT_PAD_WEST (1u << 14)
#define GAMEPKG_INPUT_PAD_NORTH (1u << 15)
#define GAMEPKG_INPUT_PAD_TOUCHPAD (1u << 16)
#define GAMEPKG_INPUT_PAD_PADDLE_1 (1u << 17)
#define GAMEPKG_INPUT_PAD_PADDLE_2 (1u << 18)
#define GAMEPKG_INPUT_PAD_PADDLE_3 (1u << 19)
#define GAMEPKG_INPUT_PAD_PADDLE_4 (1u << 20)
typedef struct gamepkg_input_event {
    uint64_t device, timestamp_ns;
    uint32_t kind, code;
    float x, y;
    uint64_t contact;
} gamepkg_input_event;
typedef struct gamepkg_input_gamepad {
    uint64_t device, timestamp_ns;
    uint32_t buttons;
    int16_t left_x, left_y, right_x, right_y;
    uint16_t left_trigger, right_trigger;
} gamepkg_input_gamepad;
typedef struct gamepkg_input_pointer {
    uint64_t device, timestamp_ns;
    float x, y;
    uint32_t buttons, reserved;
} gamepkg_input_pointer;
typedef struct gamepkg_input_contact {
    uint64_t device, timestamp_ns;
    uint64_t contact;
    float x, y;
} gamepkg_input_contact;
typedef struct gamepkg_input_device {
    uint64_t device, backend_token;
    uint32_t kind, backend;
    int32_t player;
    uint32_t reserved;
} gamepkg_input_device;
#ifdef __cplusplus
extern "C" {
#endif
/* Events are 40 bytes; snapshots/descriptors are 32 bytes on supported
   32/64-bit ABIs. Device handles
   belong to one native process and are never reused after disconnect.
   All calls are synchronized; no OS event pump or callback runs inside.
   A player slot is claimed on a fresh pad button press, not an axis or
   physical USB/XInput slot. Game-specific mappings remain in the adapter. */
/* Returns 1 connected, 0 absent, -1 invalid. Absent writes a zero state.
   While unfocused, connected pads report neutral controls. After reset or
   focus return, held buttons need release before another press is delivered. */
int32_t gamepkg_input_gamepad_state(uint32_t player, gamepkg_input_gamepad *out);
/* Keyboard codes are USB HID keyboard usages 1..511. Returns 0/1 or -1. */
int32_t gamepkg_input_key_state(uint64_t device, uint32_t code);
int32_t gamepkg_input_pointer_state(uint64_t device, gamepkg_input_pointer *out);
/* Ordered events, without OS key repeats. Pointer positions are window
   pixels, relative/wheel values retain backend units, touch XY is 0..1.
   TEXT code is a Unicode scalar. Touch uses the full 64-bit contact field
   and code zero; other events require contact zero. Never truncate a host
   finger ID. Timestamps are backend monotonic ns; zero
   means unknown. Short reads preserve the rest; invalid buffers consume
   nothing. Capacity is in records. Zero capacity reads nothing. */
int32_t gamepkg_input_read(gamepkg_input_event *out, uint32_t capacity);
/* Zero capacity queries count. Insufficient capacity writes nothing and
   returns -1. Device player is -1 until assigned. On RESET, resample held
   state and devices: queue overflow preserves current snapshots. Restore held state without
   synthesizing new press edges. Focus loss separately suppresses held
   buttons and cancels touches. Snapshot-only games need not drain events. */
int32_t gamepkg_input_devices(gamepkg_input_device *out, uint32_t capacity);
int32_t gamepkg_input_contacts(gamepkg_input_contact *out, uint32_t capacity);
#ifdef __cplusplus
}
#endif
#endif
