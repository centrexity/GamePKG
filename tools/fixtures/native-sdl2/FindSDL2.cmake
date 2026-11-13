# Maintainer override when only a target-width SDL runtime is installed locally.
if(NOT EXISTS "${GAMEPKG_TEST_SDL2_LIBRARY}" OR NOT EXISTS "${GAMEPKG_TEST_SDL2_INCLUDE}/SDL.h")
    message(FATAL_ERROR "Set valid SDL2 runtime and include paths")
endif()
if(TARGET SDL2::SDL2)
    set(SDL2_FOUND TRUE)
    return()
endif()
add_library(SDL2::SDL2 UNKNOWN IMPORTED GLOBAL)
set_target_properties(SDL2::SDL2 PROPERTIES
    IMPORTED_LOCATION "${GAMEPKG_TEST_SDL2_LIBRARY}"
    INTERFACE_INCLUDE_DIRECTORIES "${GAMEPKG_TEST_SDL2_INCLUDE}")
set(SDL2_FOUND TRUE)
