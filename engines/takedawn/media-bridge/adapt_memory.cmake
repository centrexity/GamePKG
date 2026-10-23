# A failed required alias must stop startup before generated game code executes.
# Successful mapping behavior is unchanged. Never modify the pinned checkout.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for the pinned memory adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream memory source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "c83cb0a4586ef3298911f4639e79ec1f904b4133b8469676ff0fa461b63f82f4")
    message(FATAL_ERROR "Unreviewed xboxrecomp memory layout; pinned hash differs")
endif()
file(READ "${INPUT}" source)
set(before [[            XR_ERROR( "  WARNING: tiled aperture at 0x%08X failed"
                    " (error %lu); rendering writes will fault\n",
                    XBOX_TILED_BASE, GetLastError());]])
set(after [[            XR_ERROR( "GamePKG: required tiled memory mapping at 0x%08X failed"
                    " (error %lu); stopping before game startup.\n",
                    XBOX_TILED_BASE, (unsigned long)GetLastError());
            return FALSE;]])
string(FIND "${source}" "${before}" location)
if(location LESS 0)
    message(FATAL_ERROR "Pinned tiled mapping failure branch is missing")
endif()
string(REPLACE "${before}" "${after}" adapted "${source}")
file(WRITE "${OUTPUT}.tmp" "/* GamePKG: stop cleanly when the required tiled alias cannot map. */\n${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
