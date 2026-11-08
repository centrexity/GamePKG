# Preserve the pinned clock and its rate; specialize only exact 10MHz conversion.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for the Windows clock adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream clock source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "70ae62a56934eb3edafee5fc031177df8bc714d667c72e3b8290f45ff3615d94")
    message(FATAL_ERROR "Unreviewed xboxrecomp host clock; pinned hash differs")
endif()
file(READ "${INPUT}" source)
set(before [==[    /* Split so the product never overflows: QPC runs at 10 MHz under Wine]==])
set(after [==[    /* At 10 MHz every counter tick is exactly 100 ns. Unsigned wrap is
     * identical to the split conversion, including timestamps past the
     * uint64_t nanosecond range. Keep the original path for other clocks
     * and negative counter values. No change to the clock or its rate. */
    if (freq.QuadPart == 10000000 && t.QuadPart >= 0)
        return (uint64_t)t.QuadPart * 100u;
    /* Split so the product never overflows: QPC runs at 10 MHz under Wine]==])
string(FIND "${source}" "${before}" location)
if(location LESS 0)
    message(FATAL_ERROR "Pinned host clock conversion site is missing")
endif()
string(REPLACE "${before}" "${after}" adapted "${source}")
# Keep the reviewed output identical across the Windows and POSIX CMake hosts.
file(CONFIGURE OUTPUT "${OUTPUT}" CONTENT "${adapted}" @ONLY NEWLINE_STYLE UNIX)
