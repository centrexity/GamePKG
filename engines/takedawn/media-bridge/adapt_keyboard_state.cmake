if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite pinned input source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "f0362a7875d70dabe826508c17e09719a37b4753bcc536688181e9e7178993a1")
    message(FATAL_ERROR "Unreviewed keyboard state source")
endif()
file(READ "${INPUT}" source)
function(replace_exact before after)
    string(FIND "${source}" "${before}" location)
    if(location LESS 0)
        message(FATAL_ERROR "Pinned keyboard adaptation point missing: ${before}")
    endif()
    string(REPLACE "${before}" "${after}" source "${source}")
    set(source "${source}" PARENT_SCOPE)
endfunction()
replace_exact("#include <string.h>" "#include <string.h>\n#include <stdatomic.h>")
replace_exact([=[/* Reading this needs no lock. Each entry is written only by the window's
 * thread and read only by the game's poll, one byte at a time, and a press
 * seen a poll late is indistinguishable from one made a poll later. */
static volatile unsigned char s_key_down[256];

static void (*volatile s_activity)(void);]=]
[=[/* The window writes while game threads poll. Volatile is not inter-thread
 * synchronization. Keep each legacy key atomic and publish callbacks once;
 * the common GamePKG service owns coherent multi-device snapshots. */
static _Atomic unsigned char s_key_down[256];

typedef void (*gamepkg_key_activity)(void);
static _Atomic(gamepkg_key_activity) s_activity;]=])
replace_exact("    s_activity = fn;" "    atomic_store_explicit(&s_activity, fn, memory_order_release);")
replace_exact([=[        s_key_down[vk] = down ? 1 : 0;
        if (down && s_activity && vk_mapped(vk) && kb_on())
            s_activity();]=]
[=[        atomic_store_explicit(&s_key_down[vk], down ? 1 : 0, memory_order_relaxed);
        gamepkg_key_activity activity = atomic_load_explicit(&s_activity, memory_order_acquire);
        if (down && activity && vk_mapped(vk) && kb_on())
            activity();]=])
replace_exact("    memset((void *)s_key_down, 0, sizeof s_key_down);"
    "    for (unsigned i = 0; i < 256; ++i)\n        atomic_store_explicit(&s_key_down[i], 0, memory_order_relaxed);")
replace_exact("    return s_key_down[vk] != 0;"
    "    return atomic_load_explicit(&s_key_down[vk], memory_order_relaxed) != 0;")
replace_exact("    static DWORD packet;" "    static _Atomic DWORD packet;")
replace_exact("    out->dwPacketNumber = ++packet;"
    "    out->dwPacketNumber = atomic_fetch_add_explicit(&packet, 1, memory_order_relaxed) + 1;")
file(CONFIGURE OUTPUT "${OUTPUT}" CONTENT "${source}" @ONLY NEWLINE_STYLE UNIX)
