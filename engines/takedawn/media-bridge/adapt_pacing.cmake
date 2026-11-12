# Stage only the reviewed lowered-poll notification; never edit upstream.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for the pinned pacing adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream pacing source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "20f7794c0a646c7cb8782521d8675e61bd660ccd7dcc19d345ee02b787cfc05b")
    message(FATAL_ERROR "Unreviewed xboxrecomp pacing; pinned hash differs")
endif()
file(READ "${INPUT}" source)
set(before [[    if (!g_xbox_spin_sleep)
        return;
    site = site_for(va);]])
set(after [[    if (!g_xbox_spin_sleep)
        return;
#if defined(_WIN32)
    /* Wake the command worker if this lowered poll is waiting on its flush.
     * The worker alone still decides when prior commands are consumed. */
    { extern void xbox_Nv2aKickWake(void); xbox_Nv2aKickWake(); }
#endif
    site = site_for(va);]])
string(FIND "${source}" "${before}" location)
if(location LESS 0)
    message(FATAL_ERROR "Pinned lowered spin-wait site is missing")
endif()
string(REPLACE "${before}" "${after}" adapted "${source}")
file(WRITE "${OUTPUT}.tmp" "${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
