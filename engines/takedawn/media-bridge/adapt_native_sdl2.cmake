if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite pinned input source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "316fe27024ec06ccf72cd6cd08b8602de87dd94843e2be7f99ad243eae47ae9c")
    message(FATAL_ERROR "Unreviewed native controller source")
endif()
file(READ "${INPUT}" source)
function(replace_exact before after)
    string(FIND "${source}" "${before}" location)
    if(location LESS 0)
        message(FATAL_ERROR "Pinned SDL2 adaptation point missing: ${before}")
    endif()
    string(REPLACE "${before}" "${after}" source "${source}")
    set(source "${source}" PARENT_SCOPE)
endfunction()
function(replace_section begin end replacement)
    string(FIND "${source}" "${begin}" first)
    string(FIND "${source}" "${end}" last)
    if(first LESS 0 OR last LESS first)
        message(FATAL_ERROR "Pinned SDL2 function section missing: ${begin}")
    endif()
    string(SUBSTRING "${source}" 0 ${first} prefix)
    string(SUBSTRING "${source}" ${last} -1 suffix)
    set(source "${prefix}${replacement}\n\n${suffix}" PARENT_SCOPE)
endfunction()
replace_exact("#include <time.h>" "#include <time.h>\n#include <stdatomic.h>\n#include \"native_sdl2.h\"\n#include \"native_sdl2_events.h\"")
replace_exact("case SDL_APP_DIDENTERFOREGROUND:\n        xbox_InputSetBackground(0);"
    "case SDL_APP_DIDENTERFOREGROUND:\n        xbox_InputSetBackground(SDL_GetKeyboardFocus() == NULL);")
replace_exact("static void pad_active(SDL_GameController *c)"
    "static void pad_active(SDL_GameController *c, xbox_pad_family *family, int *label_mapped)")
replace_exact("    xbox_InputSetActivePad(f, labels);"
    "    *family = f; *label_mapped = labels;")
replace_exact("static volatile int g_main_pump;" "static _Atomic int g_main_pump;")
replace_exact("int port;                         /* -1: no input yet */" "int port;                         /* -1: no input yet */\n    uint64_t device;")
replace_section("static void pad_open_index(int i)" "/* Window thread: the lowest free port" [=[static void pad_open_index(int i)
{
    SDL_JoystickID id = SDL_JoystickGetDeviceInstanceID(i);
    int free_k = -1;
    char description[160];
    if (!SDL_IsGameController(i)) return;
    pthread_mutex_lock(&g_pads_lock);
    if (g_pads_closed) { pthread_mutex_unlock(&g_pads_lock); return; }
    for (int k = 0; k < PAD_OPEN_MAX; k++) {
        if (g_open[k].c && g_open[k].id == id) {
            pthread_mutex_unlock(&g_pads_lock); return;
        }
        if (!g_open[k].c && free_k < 0) free_k = k;
    }
    if (free_k < 0) { pthread_mutex_unlock(&g_pads_lock); return; }
    SDL_GameController *controller = SDL_GameControllerOpen(i);
    if (!controller) { pthread_mutex_unlock(&g_pads_lock); return; }
    uint64_t device = gamepkg_input_connect(GAMEPKG_INPUT_GAMEPAD,
        GAMEPKG_SDL2_BACKEND, (uint64_t)(uint32_t)id);
    if (!device) {
        SDL_GameControllerClose(controller);
        pthread_mutex_unlock(&g_pads_lock); return;
    }
    g_open[free_k].c = controller; g_open[free_k].id = id;
    g_open[free_k].port = -1; g_open[free_k].device = device;
    pad_describe(controller, description, sizeof description);
#if defined(__APPLE__) && SDL_VERSION_ATLEAST(2, 24, 0)
    const char *path = SDL_GameControllerPath(controller);
    size_t length = strlen(description);
    snprintf(description + length, sizeof description - length, " [%s]",
        path && *path ? "HIDAPI" : "GameController framework");
#endif
    XR_NOTICE("[INPUT] controller %s opened; it gets a port at its first input\n", description);
    pthread_mutex_unlock(&g_pads_lock);
}]=])
replace_section("static void pad_claim(SDL_JoystickID id)" "void xbox_InputPadEvent(const SDL_Event *ev)" [=[/* Under the engine's existing pad lock. SDL's window thread still owns all
 * opening/closing and the one event pump. State reads add no OS event pump. */
static int gamepkg_sdl2_pending_family = -1, gamepkg_sdl2_pending_labels;
static uint64_t gamepkg_sdl2_last_ns;
static int gamepkg_sdl2_last_port = XBOX_MAX_CONTROLLERS;
static void gamepkg_sdl2_refresh(SDL_JoystickID pressed_id, int pressed_button, int requested_port)
{
    uint64_t now = gamepkg_input_time_ns();
    if (requested_port >= 0 && requested_port > gamepkg_sdl2_last_port &&
        now >= gamepkg_sdl2_last_ns && now - gamepkg_sdl2_last_ns < UINT64_C(1000000)) {
        gamepkg_sdl2_last_port = requested_port;
        return;
    }
    gamepkg_sdl2_last_ns = now;
    gamepkg_sdl2_last_port = requested_port < 0 ? XBOX_MAX_CONTROLLERS : requested_port;
    /* A known queued button event keeps its original ordering, ahead of
     * other controllers whose current snapshots also contain a press. */
    if (pressed_button >= 0) for (int k = 0; k < PAD_OPEN_MAX; k++)
        if (g_open[k].c && g_open[k].id == pressed_id && g_open[k].device)
            gamepkg_sdl2_sample(g_open[k].c, g_open[k].device, pressed_button);
    for (int k = 0; k < PAD_OPEN_MAX; k++) {
        if (!g_open[k].c) continue;
        if (!SDL_GameControllerGetAttached(g_open[k].c)) {
            if (g_open[k].device) gamepkg_input_disconnect(g_open[k].device);
            g_open[k].device = 0;
            continue;
        }
        if (g_open[k].id != pressed_id || pressed_button < 0)
            gamepkg_sdl2_sample(g_open[k].c, g_open[k].device, -1);
    }
    for (int p = 0; p < XBOX_MAX_CONTROLLERS; p++) {
        gamepkg_input_gamepad report;
        g_pads[p] = NULL;
        if (gamepkg_input_gamepad_state((uint32_t)p, &report) != 1) continue;
        for (int k = 0; k < PAD_OPEN_MAX; k++) {
            if (g_open[k].c && g_open[k].device == report.device) {
                g_pads[p] = g_open[k].c;
                if (g_open[k].port != p) {
                    g_open[k].port = p;
                    if (!g_main_pump) {
                        xbox_pad_family family;
                        pad_active(g_open[k].c, &family, &gamepkg_sdl2_pending_labels);
                        gamepkg_sdl2_pending_family = (int)family;
                    }
                    XR_NOTICE("[INPUT] controller instance %d -> port %d, shared first-button ownership\n",
                        (int)g_open[k].id, p);
                }
                break;
            }
        }
    }
}
static void pad_claim(SDL_JoystickID id, int button)
{
    xbox_pad_family family = XBOX_PAD_FAMILY_XBOX;
    int active = 0, labels = 0;
    pthread_mutex_lock(&g_pads_lock);
    if (!g_pads_closed) {
        gamepkg_sdl2_refresh(id, button, -1);
        for (int k = 0; k < PAD_OPEN_MAX; k++)
            if (g_open[k].c && g_open[k].id == id) {
                pad_active(g_open[k].c, &family, &labels); active = 1;
            }
    }
    pthread_mutex_unlock(&g_pads_lock);
    /* A glyph listener may itself poll input. Metadata is copied while the
     * handle is protected, then the listener runs outside the pad lock. */
    if (active) xbox_InputSetActivePad(family, labels);
}]=])
replace_exact("pad_claim(ev->cbutton.which);" "pad_claim(ev->cbutton.which, ev->cbutton.button);")
replace_exact([=[        for (int k = 0; k < PAD_OPEN_MAX; k++)
            if (g_open[k].c && g_open[k].id == ev->cbutton.which)
                pad_active(g_open[k].c);]=] "")
replace_section("case SDL_CONTROLLERDEVICEREMOVED:" "case SDL_CONTROLLERBUTTONDOWN:" [=[case SDL_CONTROLLERDEVICEREMOVED: {
        int removed = 0, port = -1;
        pthread_mutex_lock(&g_pads_lock);
        for (int k = 0; k < PAD_OPEN_MAX; k++) {
            if (!g_open[k].c || g_open[k].id != ev->cdevice.which) continue;
            removed = 1; port = g_open[k].port;
            if (port >= 0) g_pads[port] = NULL;
            if (g_open[k].device) gamepkg_input_disconnect(g_open[k].device);
            SDL_GameControllerClose(g_open[k].c);
            /* Publish NULL before releasing the same lock used by polling.
             * No worker can observe a closed SDL handle. */
            g_open[k].c = NULL; g_open[k].device = 0; g_open[k].port = -1;
            break;
        }
        pthread_mutex_unlock(&g_pads_lock);
        if (removed) XR_NOTICE("[INPUT] controller on port %d removed; its shared slot is free\n", port);
        break;
    }
    ]=])
replace_section("static void open_controllers(void)" "static void sdl_init(void)" [=[static void open_controllers(void)
{
    /* Headless mode has one SDL owner and no removal events consumer. */
    pthread_mutex_lock(&g_pads_lock);
    if (g_pads_closed) { pthread_mutex_unlock(&g_pads_lock); return; }
    for (int k = 0; k < PAD_OPEN_MAX; k++) {
        if (!g_open[k].c || SDL_GameControllerGetAttached(g_open[k].c)) continue;
        if (g_open[k].device) gamepkg_input_disconnect(g_open[k].device);
        if (g_open[k].port >= 0) g_pads[g_open[k].port] = NULL;
        SDL_GameControllerClose(g_open[k].c);
        g_open[k].device = 0; g_open[k].c = NULL;
    }
    pthread_mutex_unlock(&g_pads_lock);
    for (int i = 0; i < SDL_NumJoysticks(); i++) pad_open_index(i);
}]=])
replace_exact("    if (g_main_pump) {\n        /* The window's thread" [=[    gamepkg_input_focus(!g_main_pump || SDL_GetKeyboardFocus() != NULL);
    if (g_main_pump) {
        /* The window's thread]=])
replace_exact([=[    pthread_mutex_lock(&g_pads_lock);
    open_controllers();
    pad_active(g_pads[0]);
    pthread_mutex_unlock(&g_pads_lock);]=] [=[    open_controllers();]=])
replace_section("static DWORD sdl_get_state(DWORD port, XBOX_INPUT_STATE *out)" "/* The motors' levels, after the rumble curve" [=[static DWORD sdl_get_state(DWORD port, XBOX_INPUT_STATE *out)
{
    gamepkg_input_gamepad report;
    XBOX_HOST_SDL_GAMEPAD pad;
    if (port >= XBOX_MAX_CONTROLLERS) return ERROR_DEVICE_NOT_CONNECTED;
    if (!out) return 160u; /* ERROR_BAD_ARGUMENTS */
    /* Headless mode has no other SDL user. A window keeps enumeration and
     * its event pump exclusively on its own thread, just as upstream. */
    if (!g_main_pump) open_controllers();
    pthread_mutex_lock(&g_pads_lock);
    if (g_pads_closed) {
        pthread_mutex_unlock(&g_pads_lock);
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    if (!g_main_pump) SDL_GameControllerUpdate();
    gamepkg_sdl2_refresh(-1, -1, (int)port);
    if (gamepkg_input_gamepad_state(port, &report) != 1 || !g_pads[port]) {
        pthread_mutex_unlock(&g_pads_lock);
        return ERROR_DEVICE_NOT_CONNECTED;
    }
    gamepkg_sdl2_unpack(&report, &pad);
    out->dwPacketNumber = ++g_packet[port];
    int family = gamepkg_sdl2_pending_family, labels = gamepkg_sdl2_pending_labels;
    gamepkg_sdl2_pending_family = -1;
    pthread_mutex_unlock(&g_pads_lock);
    if (family >= 0) xbox_InputSetActivePad((xbox_pad_family)family, labels);
    if (xbox_InputFaceLayoutNintendo()) xbox_InputMirrorFaceSdl(&pad);
    xbox_InputMapSdl(&pad, &out->Gamepad);
    return ERROR_SUCCESS;
}]=])
replace_exact("        g_open[k].c = NULL;\n    }\n    for (int p" [=[        if (g_open[k].device) gamepkg_input_disconnect(g_open[k].device);
        g_open[k].device = 0;
        g_open[k].c = NULL;
    }
    for (int p]=])
replace_exact("    pthread_mutex_unlock(&g_pads_lock);\n}\n\nstatic uint64_t sdl_now_ms(void)"
    "    pthread_mutex_unlock(&g_pads_lock);\n    gamepkg_sdl2_window_shutdown();\n}\n\nstatic uint64_t sdl_now_ms(void)")
file(CONFIGURE OUTPUT "${OUTPUT}" CONTENT "${source}" @ONLY NEWLINE_STYLE UNIX)
