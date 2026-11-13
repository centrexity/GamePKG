cmake_minimum_required(VERSION 3.20)
project(gamepkg_sdl2_integration C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
if(GAMEPKG_TEST_SDL2_LIBRARY)
    list(PREPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}")
endif()
set(XR_SDL_HOST ON)
if(WIN32)
    # Exercise the same SDL backend used by the Windows Vulkan game. This
    # fixture enters through a console main rather than SDL's window shim.
    set(XBOXRECOMP_WIN_VULKAN ON)
    add_compile_definitions(XBOXRECOMP_WIN_VULKAN=1 SDL_MAIN_HANDLED=1)
endif()
add_compile_options(-include "${SDK}/src/platform/xr_log.h")
add_subdirectory("${SDK}/src/platform" platform)
add_subdirectory("${SDK}/src/input" input)
include("${BRIDGE}/native_input.cmake")
gamepkg_attach_native_input()
if(WIN32)
    # The full game obtains this from its Vulkan runtime. Keep the fixture
    # equally self-contained, without requiring a winpthread DLL on players.
    target_link_libraries(xbox_input PUBLIC -Wl,-Bstatic winpthread unwind -Wl,-Bdynamic)
endif()
add_executable(gamepkg_native_sdl2 "${BRIDGE}/tests/native_sdl2.c")
target_include_directories(gamepkg_native_sdl2 PRIVATE "${BRIDGE}")
target_link_libraries(gamepkg_native_sdl2 PRIVATE xbox_input)
target_compile_options(gamepkg_native_sdl2 PRIVATE -Wall -Wextra -Werror)
add_executable(gamepkg_native_sdl2_events "${BRIDGE}/tests/native_sdl2_events.c")
target_include_directories(gamepkg_native_sdl2_events PRIVATE "${BRIDGE}")
target_link_libraries(gamepkg_native_sdl2_events PRIVATE xbox_input)
target_compile_options(gamepkg_native_sdl2_events PRIVATE -Wall -Wextra -Werror)
add_executable(gamepkg_native_sdl2_hotplug "${BRIDGE}/tests/native_sdl2_hotplug.c")
target_include_directories(gamepkg_native_sdl2_hotplug PRIVATE "${BRIDGE}")
target_link_libraries(gamepkg_native_sdl2_hotplug PRIVATE xbox_input)
target_compile_options(gamepkg_native_sdl2_hotplug PRIVATE -Wall -Wextra -Werror)
# Upstream's headless rumble fixture assumed device-order ownership. Supply a
# first button through its already-open virtual joystick. All original rumble,
# timing, process-exit and stop/close assertions remain unchanged.
if(TARGET input_rumble_sdl_test)
file(READ "${SDK}/tests/input_rumble_sdl/test_main.c" rumble)
set(join_point "    return 1;\n}\n\nenum { END_EXIT")
string(FIND "${rumble}" "${join_point}" join_at)
if(join_at LESS 0)
    message(FATAL_ERROR "Pinned virtual rumble setup is missing")
endif()
set(join [=[    xbox_InputInit();
    SDL_Joystick *join_pad = SDL_JoystickFromInstanceID(SDL_JoystickGetDeviceInstanceID(0));
    if (!join_pad || SDL_JoystickSetVirtualButton(join_pad, SDL_CONTROLLER_BUTTON_A, 1) != 0)
        return 0;
    SDL_GameControllerUpdate();
    XBOX_INPUT_STATE join_state;
    if (xbox_InputGetState(0, &join_state) != ERROR_SUCCESS)
        return 0;
    return 1;
}

enum { END_EXIT]=])
string(REPLACE "${join_point}" "${join}" rumble "${rumble}")
file(CONFIGURE OUTPUT "${CMAKE_BINARY_DIR}/rumble_join.c" CONTENT "${rumble}" @ONLY NEWLINE_STYLE UNIX)
set_property(TARGET input_rumble_sdl_test PROPERTY SOURCES "${CMAKE_BINARY_DIR}/rumble_join.c")
endif()
foreach(mutation IN ITEMS buttons y)
    if(GAMEPKG_NEGATIVE_HEADERS)
        add_executable(negative_${mutation} "${BRIDGE}/tests/native_sdl2.c")
        target_compile_definitions(negative_${mutation} PRIVATE
            "GAMEPKG_SDL2_TEST_HEADER=\"${GAMEPKG_NEGATIVE_HEADERS}/${mutation}.h\"")
        target_link_libraries(negative_${mutation} PRIVATE xbox_input)
    endif()
endforeach()
if(GAMEPKG_NEGATIVE_HEADERS)
    get_target_property(input_sources xbox_input SOURCES)
    foreach(mutation IN ITEMS gate identity)
        set(mutated_sources)
        foreach(source IN LISTS input_sources)
            if(NOT IS_ABSOLUTE "${source}")
                set(source "${SDK}/src/input/${source}")
            endif()
            if((mutation STREQUAL "gate" AND source MATCHES "/keyboard_sdl\\.c$") OR
               (mutation STREQUAL "identity" AND source MATCHES "/touch_pad_sdl\\.c$"))
                file(READ "${source}" contents)
                if(mutation STREQUAL "gate")
                    set(before "if (disposition == GAMEPKG_SDL2_EVENT_DROP) return;")
                    set(after "/* negative control: ignore rejected input */")
                else()
                    set(before "gamepkg_sdl2_layout_finger(&ev->tfinger)")
                    set(after "(int64_t)ev->tfinger.fingerId")
                endif()
                string(FIND "${contents}" "${before}" location)
                if(location LESS 0)
                    message(FATAL_ERROR "Window negative control point is missing")
                endif()
                string(REPLACE "${before}" "${after}" contents "${contents}")
                set(source "${CMAKE_BINARY_DIR}/negative_${mutation}_source.c")
                file(CONFIGURE OUTPUT "${source}" CONTENT "${contents}" @ONLY NEWLINE_STYLE UNIX)
            endif()
            list(APPEND mutated_sources "${source}")
        endforeach()
        add_library(negative_${mutation}_backend STATIC ${mutated_sources})
        target_include_directories(negative_${mutation}_backend PRIVATE "${BRIDGE}")
        # Reuse the real target's transitive platform/SDL/Rust dependencies.
        # This clone defines the full backend, so original backend objects
        # are not selected from the archive by the static linker.
        target_link_libraries(negative_${mutation}_backend PUBLIC xbox_input)
        add_executable(negative_${mutation} "${BRIDGE}/tests/native_sdl2_events.c")
        target_include_directories(negative_${mutation} PRIVATE "${BRIDGE}")
        target_link_libraries(negative_${mutation} PRIVATE negative_${mutation}_backend)
    endforeach()
endif()
