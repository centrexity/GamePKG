# Apply only to a pinned main.c (or its already verified Windows adaptation).
function(gamepkg_takedawn_main_presentation input_name output_name)
    set(source "${${input_name}}")
    string(REGEX MATCHALL "xbox_HostWindowSetTitle\\(BO3_APP_NAME\\)" titles "${source}")
    list(LENGTH titles title_count)
    string(REGEX MATCHALL "xbox_SettingsInit\\(BO3_USER_DIR\\)" settings "${source}")
    list(LENGTH settings settings_count)
    if(NOT title_count EQUAL 2 OR NOT settings_count EQUAL 1)
        message(FATAL_ERROR "Pinned game title/default sites differ")
    endif()
    string(REPLACE "xbox_HostWindowSetTitle(BO3_APP_NAME)"
        "xbox_HostWindowSetTitle(\"Burnout 3: Takedown (XBOX version)\")" source "${source}")
    # Saved settings have already loaded; environment and saved choices win.
    # Keep headless maintainer runs at their original graphics defaults.
    string(REPLACE "    xbox_SettingsInit(BO3_USER_DIR);"
        "    xbox_SettingsInit(BO3_USER_DIR);\n    if (!headless) {\n        env_default(\"RECOMP_RENDER_SCALE\", \"2\");\n        env_default(\"RECOMP_DISPLAY_WIDESCREEN\", \"1\");\n        env_default(\"RECOMP_GFX_HUD_FIX\", \"1\");\n        env_default(\"RECOMP_GFX_AA\", \"smaa\");\n        env_default(\"RECOMP_GFX_SHARPEN\", \"0.10\");\n        env_default(\"RECOMP_GFX_LOD_BIAS\", \"auto\");\n        env_default(\"RECOMP_GFX_UI_SCALE\", \"2x\");\n    }"
        source "${source}")
    # The signed host forwards structured launch arguments; validate them
    # before the original game's argument handling or any ISO/window work.
    set(before "    for (i = 1; i < argc; i++) {\n        if (!strcmp(argv[i], \"--headless\"))")
    string(FIND "${source}" "${before}" site)
    if(site LESS 0)
        message(FATAL_ERROR "Pinned native argument loop differs")
    endif()
    string(REPLACE "${before}" "    for (i = 1; i < argc; i++) {\n        int launch_arg = gamepkg_launch_option(argc, argv, &i);\n        if (launch_arg < 0) return 2;\n        if (launch_arg) { recomp_env_reload(); continue; }\n        if (!strcmp(argv[i], \"--headless\"))" source "${source}")
    file(TO_CMAKE_PATH "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/launch_args.h" launch_header)
    set(source "static int gamepkg_launch_option(int, char **, int *);\n${source}\n#include \"${launch_header}\"\n")
    set(${output_name} "${source}" PARENT_SCOPE)
endfunction()
