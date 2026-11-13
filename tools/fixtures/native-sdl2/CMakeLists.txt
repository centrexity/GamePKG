cmake_minimum_required(VERSION 3.20)
project(gamepkg_sdl2_integration C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
if(GAMEPKG_TEST_SDL2_LIBRARY)
    list(PREPEND CMAKE_MODULE_PATH "${CMAKE_CURRENT_SOURCE_DIR}")
endif()
set(XR_SDL_HOST ON)
add_compile_options(-include "${SDK}/src/platform/xr_log.h")
add_subdirectory("${SDK}/src/platform" platform)
add_subdirectory("${SDK}/src/input" input)
include("${BRIDGE}/native_input.cmake")
gamepkg_attach_native_input()
add_executable(gamepkg_native_sdl2 "${BRIDGE}/tests/native_sdl2.c")
target_include_directories(gamepkg_native_sdl2 PRIVATE "${BRIDGE}")
target_link_libraries(gamepkg_native_sdl2 PRIVATE xbox_input)
target_compile_options(gamepkg_native_sdl2 PRIVATE -Wall -Wextra -Werror)
# Upstream's headless rumble fixture assumed device-order ownership. Supply a
# first button through its already-open virtual joystick. All original rumble,
# timing, process-exit and stop/close assertions remain unchanged.
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
foreach(mutation IN ITEMS buttons y)
    if(GAMEPKG_NEGATIVE_HEADERS)
        add_executable(negative_${mutation} "${BRIDGE}/tests/native_sdl2.c")
        target_compile_definitions(negative_${mutation} PRIVATE
            "GAMEPKG_SDL2_TEST_HEADER=\"${GAMEPKG_NEGATIVE_HEADERS}/${mutation}.h\"")
        target_link_libraries(negative_${mutation} PRIVATE xbox_input)
    endif()
endforeach()
