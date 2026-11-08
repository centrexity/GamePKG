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
    # Keep headless maintainer runs at their original stock resolution.
    string(REPLACE "    xbox_SettingsInit(BO3_USER_DIR);"
        "    xbox_SettingsInit(BO3_USER_DIR);\n    if (!headless)\n        env_default(\"RECOMP_RENDER_SCALE\", \"2\");"
        source "${source}")
    set(${output_name} "${source}" PARENT_SCOPE)
endfunction()
