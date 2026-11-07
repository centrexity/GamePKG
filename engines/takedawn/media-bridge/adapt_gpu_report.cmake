# Rank GPU diagnostics without invalidating method-to-counter lookup slots.
# Only the pinned report block changes; drawing and method dispatch are untouched.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for pinned GPU report adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream GPU source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "efc81258a81c3a2add67b904e4815b53e2283243e000c768b97c8319e2c1d220")
    message(FATAL_ERROR "Unreviewed GPU executor source; pinned hash does not match")
endif()
file(READ "${INPUT}" source)
set(before [==[        int shown = recomp_env(RENV_PB_UNHANDLED_ALL) ? s_unhandled_count : 10;
    for (i = 0; i < shown && i < s_unhandled_count; i++) {
        int best = i;
        for (j = i + 1; j < s_unhandled_count; j++)
            if (s_unhandled[j].count > s_unhandled[best].count)
                best = j;
        if (best != i) {
            PbUnhandled t = s_unhandled[i];
            s_unhandled[i] = s_unhandled[best];
            s_unhandled[best] = t;
        }
        {
            /* The value as well as the count. A method nobody decoded is
             * a guess until you see what it carried: screen coordinates,
             * a 0..1 texcoord and a packed colour are told apart at a
             * glance, and that is what says which vertex encoding a title
             * is using. */
            union { uint32_t u; float f; } v;
            v.u = s_unhandled[i].last_param;
            fprintf(stderr, "  [GPU]   0x%04X x%-8u last=0x%08X (%.4f)\n",
                    s_unhandled[i].method, s_unhandled[i].count,
                    v.u, v.f);
        }
    }]==])
set(after [==[        int shown = recomp_env(RENV_PB_UNHANDLED_ALL) ? s_unhandled_count : 10;
        /* Keep first-seen slots stable: reports rank a snapshot. */
        PbUnhandled ranked[PB_EXEC_MAX_UNHANDLED];
        memcpy(ranked, s_unhandled, (size_t)s_unhandled_count * sizeof(ranked[0]));
    for (i = 0; i < shown && i < s_unhandled_count; i++) {
        int best = i;
        for (j = i + 1; j < s_unhandled_count; j++)
            if (ranked[j].count > ranked[best].count)
                best = j;
        if (best != i) {
            PbUnhandled t = ranked[i];
            ranked[i] = ranked[best];
            ranked[best] = t;
        }
        {
            /* The value as well as the count. A method nobody decoded is
             * a guess until you see what it carried: screen coordinates,
             * a 0..1 texcoord and a packed colour are told apart at a
             * glance, and that is what says which vertex encoding a title
             * is using. */
            union { uint32_t u; float f; } v;
            v.u = ranked[i].last_param;
            fprintf(stderr, "  [GPU]   0x%04X x%-8u last=0x%08X (%.4f)\n",
                    ranked[i].method, ranked[i].count,
                    v.u, v.f);
        }
    }]==])
string(REPLACE "${before}" "${after}" adapted "${source}")
if(adapted STREQUAL source)
    message(FATAL_ERROR "Pinned GPU report block is missing")
endif()
file(WRITE "${OUTPUT}.tmp" "${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
