# Maintainer reference build only. Configure the pinned Takedawn tree with
# -DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=/absolute/path/to/this/attach.cmake
# after building this directory's Rust static library. No upstream source edit
# or asset extraction is needed. The reference executable keeps its own name.
function(gamepkg_attach_input_glyphs)
    if(NOT TARGET burnout3)
        message(FATAL_ERROR "The pinned Takedawn burnout3 target is missing")
    endif()
    get_target_property(game_dir burnout3 SOURCE_DIR)
    get_target_property(game_sources burnout3 SOURCES)
    file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/gamepkg-media")
    foreach(kind IN ITEMS glyphs controls)
        list(FIND game_sources "${kind}.c" source_index)
        if(source_index LESS 0)
            message(FATAL_ERROR "Pinned ${kind}.c target source is missing")
        endif()
        set(staged "${CMAKE_BINARY_DIR}/gamepkg-media/${kind}.c")
        execute_process(COMMAND "${CMAKE_COMMAND}"
            "-DINPUT=${game_dir}/${kind}.c" "-DOUTPUT=${staged}" "-DKIND=${kind}"
            -P "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/adapt_input_glyphs.cmake"
            COMMAND_ERROR_IS_FATAL ANY)
        list(REMOVE_AT game_sources ${source_index})
        list(APPEND game_sources "${staged}")
    endforeach()
    set_property(TARGET burnout3 PROPERTY SOURCES "${game_sources}")
endfunction()

function(gamepkg_attach_presentation_settings)
    get_target_property(overlay_dir xbox_overlay SOURCE_DIR)
    get_target_property(overlay_sources xbox_overlay SOURCES)
    list(FIND overlay_sources xbox_settings.c settings_index)
    if(settings_index LESS 0)
        message(FATAL_ERROR "Pinned overlay settings target entry is missing")
    endif()
    file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/gamepkg-media")
    set(settings "${CMAKE_BINARY_DIR}/gamepkg-media/xbox_settings.c")
    execute_process(COMMAND "${CMAKE_COMMAND}" "-DINPUT=${overlay_dir}/xbox_settings.c"
        "-DOUTPUT=${settings}" -DKIND=settings -P "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/adapt_presentation.cmake"
        COMMAND_ERROR_IS_FATAL ANY)
    list(REMOVE_AT overlay_sources ${settings_index})
    list(APPEND overlay_sources "${settings}")
    set_property(TARGET xbox_overlay PROPERTY SOURCES "${overlay_sources}")
    target_include_directories(xbox_overlay PRIVATE "${overlay_dir}")
endfunction()

function(gamepkg_attach_disc_media)
    gamepkg_attach_input_glyphs()
    gamepkg_attach_presentation_settings()
    if(CMAKE_SYSTEM_NAME STREQUAL "Windows")
        include("${CMAKE_CURRENT_FUNCTION_LIST_DIR}/windows_attach.cmake")
        gamepkg_attach_windows_disc_media()
        return()
    endif()
    if(NOT CMAKE_SYSTEM_NAME STREQUAL "Linux")
        message(FATAL_ERROR "The reference POSIX media adapter currently requires Linux")
    endif()
    if(NOT TARGET burnout3)
        message(FATAL_ERROR "The pinned Takedawn burnout3 target is missing")
    endif()
    set(bridge "${CMAKE_CURRENT_FUNCTION_LIST_DIR}")
    get_target_property(game_dir burnout3 SOURCE_DIR)
    get_target_property(game_sources burnout3 SOURCES)
    list(FIND game_sources main.c main_index)
    if(main_index LESS 0)
        message(FATAL_ERROR "Pinned game main target entry is missing")
    endif()
    set(main "${CMAKE_BINARY_DIR}/gamepkg-media/main.c")
    execute_process(COMMAND "${CMAKE_COMMAND}" "-DINPUT=${game_dir}/main.c"
        "-DOUTPUT=${main}" -DKIND=main -P "${bridge}/adapt_presentation.cmake" COMMAND_ERROR_IS_FATAL ANY)
    list(REMOVE_AT game_sources ${main_index})
    list(APPEND game_sources "${main}")
    set_property(TARGET burnout3 PROPERTY SOURCES "${game_sources}")
    target_include_directories(burnout3 PRIVATE "${game_dir}")

    # DVD files and Xbox HDD files are distinct devices. Upstream's general
    # Partition1 rule points at game_dir, a folder writable in its extracted
    # install. Our game_dir is a read-only image, so stage the pinned rule with
    # a dedicated writable HDD root, preserving UDATA/TDATA precedence.
    get_target_property(kernel_dir xbox_kernel SOURCE_DIR)
    file(MAKE_DIRECTORY "${CMAKE_BINARY_DIR}/gamepkg-media")
    set(adapted_path "${CMAKE_BINARY_DIR}/gamepkg-media/kernel_path.c")
    execute_process(COMMAND "${CMAKE_COMMAND}"
        "-DINPUT=${kernel_dir}/kernel_path.c" "-DOUTPUT=${adapted_path}"
        -P "${bridge}/adapt_paths.cmake" COMMAND_ERROR_IS_FATAL ANY)
    get_target_property(kernel_sources xbox_kernel SOURCES)
    list(FIND kernel_sources "kernel_path.c" path_index)
    if(path_index LESS 0)
        message(FATAL_ERROR "Pinned kernel path source target is missing")
    endif()
    list(REMOVE_AT kernel_sources ${path_index})
    list(APPEND kernel_sources "${adapted_path}")
    list(FIND kernel_sources "xbox_memory_layout.c" memory_index)
    if(memory_index LESS 0)
        message(FATAL_ERROR "Pinned memory layout source target is missing")
    endif()
    set(adapted_memory "${CMAKE_BINARY_DIR}/gamepkg-media/xbox_memory_layout.c")
    execute_process(COMMAND "${CMAKE_COMMAND}"
        "-DINPUT=${kernel_dir}/xbox_memory_layout.c" "-DOUTPUT=${adapted_memory}"
        -P "${bridge}/adapt_memory.cmake" COMMAND_ERROR_IS_FATAL ANY)
    list(REMOVE_AT kernel_sources ${memory_index})
    list(APPEND kernel_sources "${adapted_memory}")
    list(FIND kernel_sources "kernel_bridge.c" thread_index)
    if(thread_index LESS 0)
        message(FATAL_ERROR "Pinned kernel bridge source target is missing")
    endif()
    set(adapted_threads "${CMAKE_BINARY_DIR}/gamepkg-media/kernel_bridge.c")
    execute_process(COMMAND "${CMAKE_COMMAND}"
        "-DINPUT=${kernel_dir}/kernel_bridge.c" "-DOUTPUT=${adapted_threads}"
        -P "${bridge}/adapt_threads.cmake" COMMAND_ERROR_IS_FATAL ANY)
    list(REMOVE_AT kernel_sources ${thread_index})
    list(APPEND kernel_sources "${adapted_threads}")
    list(FIND kernel_sources "nv2a_pb_exec.c" gpu_report_index)
    if(gpu_report_index LESS 0)
        message(FATAL_ERROR "Pinned GPU executor source target is missing")
    endif()
    set(adapted_gpu_report "${CMAKE_BINARY_DIR}/gamepkg-media/nv2a_pb_exec.c")
    execute_process(COMMAND "${CMAKE_COMMAND}"
        "-DINPUT=${kernel_dir}/nv2a_pb_exec.c" "-DOUTPUT=${adapted_gpu_report}"
        -P "${bridge}/adapt_gpu_report.cmake" COMMAND_ERROR_IS_FATAL ANY)
    list(REMOVE_AT kernel_sources ${gpu_report_index})
    list(APPEND kernel_sources "${adapted_gpu_report}")
    set_property(TARGET xbox_kernel PROPERTY SOURCES "${kernel_sources}")
    # Correct the smoke stream's explicit screen-space transform setup, then
    # demand real colors as well as CPU/GPU agreement. Runtime state is unchanged.
    if(TARGET nv2a_backend_smoke)
        get_target_property(smoke_dir nv2a_backend_smoke SOURCE_DIR)
        get_target_property(smoke_sources nv2a_backend_smoke SOURCES)
        if(NOT smoke_sources STREQUAL "smoke.c")
            message(FATAL_ERROR "Unreviewed smoke target sources; refusing to drop any source")
        endif()
        set(adapted_smoke "${CMAKE_BINARY_DIR}/gamepkg-media/smoke.c")
        execute_process(COMMAND "${CMAKE_COMMAND}"
            "-DINPUT=${smoke_dir}/smoke.c" "-DOUTPUT=${adapted_smoke}"
            -P "${bridge}/adapt_smoke.cmake" COMMAND_ERROR_IS_FATAL ANY)
        set_property(TARGET nv2a_backend_smoke PROPERTY SOURCES "${adapted_smoke}")
    endif()
    set(archive "${bridge}/target/release/libgamepkg_takedawn_media.a")
    if(NOT EXISTS "${archive}")
        message(FATAL_ERROR "Build the GamePKG media-bridge Rust static library first")
    endif()
    target_sources(burnout3 PRIVATE "${bridge}/posix.c" "${bridge}/engine.c")
    target_link_libraries(burnout3 PRIVATE "${archive}" dl m pthread)
    foreach(symbol IN ITEMS main SDL_RWFromFile open close read write lseek fstat stat lstat access opendir readdir closedir fopen)
        target_link_options(burnout3 PRIVATE "LINKER:--wrap=${symbol}")
    endforeach()
    set_target_properties(burnout3 PROPERTIES OUTPUT_NAME "burnout3-gamepkg-media")
endfunction()
cmake_language(DEFER CALL gamepkg_attach_disc_media)
