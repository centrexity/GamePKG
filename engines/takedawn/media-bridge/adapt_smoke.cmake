# The upstream smoke stream expects screen-space input but leaves CMAT zero.
# Stage explicit test setup; never change a runtime default or pixel tolerance.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for the pinned smoke adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream smoke source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "e15db5164a7d319f9fbb98631ee598a0636d5b99fa660ba54ed491e6abfee66e")
    message(FATAL_ERROR "Unreviewed nv2a_backend_smoke source; pinned hash does not match")
endif()
file(READ "${INPUT}" source)
set(init_before [[    /* The index limit is the walker's alone (CPU path), so it runs before]])
set(init_after [[    /* GamePKG: these screen-space fixtures require an identity CMAT. The
     * runtime correctly starts with zero context, which collapses their inline
     * quads. The omission control deliberately restores that broken fixture. */
    if (!getenv("GAMEPKG_SMOKE_OMIT_COMPOSITE"))
        identity_composite();
    /* The index limit is the walker's alone (CPU path), so it runs before]])
string(REPLACE "${init_before}" "${init_after}" adapted "${source}")
if(adapted STREQUAL source)
    message(FATAL_ERROR "Pinned smoke initialization anchor is missing")
endif()
set(probes_before [[    const struct probe *p = sc == SC_BIGIDX ? s_probe_bigidx : s_probe_elem32;
    uint32_t n = sc == SC_BIGIDX ? sizeof s_probe_bigidx / sizeof *p
                                 : sizeof s_probe_elem32 / sizeof *p, i;
    int ok = 1;

    if (sc != SC_BIGIDX && sc != SC_ELEM32)
        return 1;]])
set(probes_after [[    /* GamePKG: whole-frame agreement alone also accepted two empty scenes.
     * These points are outside the textured/blended quads in all four basic
     * scenes and pin the solid quad, clear background and fractional quad. */
    static const struct probe basic[] = {
        {100, 200, 0xFFFF0000u}, {20, 20, 0xFF603010u}, {100, 400, 0xFF00C040u}
    };
    const struct probe *p = sc < SC_BIGIDX ? basic : sc == SC_BIGIDX ? s_probe_bigidx : s_probe_elem32;
    uint32_t n = sc < SC_BIGIDX ? sizeof basic / sizeof *p
                  : sc == SC_BIGIDX ? sizeof s_probe_bigidx / sizeof *p
                                   : sizeof s_probe_elem32 / sizeof *p, i;
    int ok = 1;]])
string(REPLACE "${probes_before}" "${probes_after}" with_probes "${adapted}")
if(with_probes STREQUAL adapted)
    message(FATAL_ERROR "Pinned smoke probe anchor is missing")
endif()
file(WRITE "${OUTPUT}.tmp" "/* GamePKG: pinned smoke fixture setup and stronger pixel probes. */\n${with_probes}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
