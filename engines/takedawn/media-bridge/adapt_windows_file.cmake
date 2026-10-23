# Maintainer Windows runtime staging. Never edit the original upstream source.
# The player kit is unchanged until complete Windows linking/gameplay qualifies.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for the pinned Windows file adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream file source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "9cb2318f418204766e26ab0bbc82110dbb2b248c07aea325edd956f9795e2c96")
    message(FATAL_ERROR "Unreviewed xboxrecomp kernel_file.c; pinned hash does not match")
endif()
file(READ "${INPUT}" source)
set(adapted "${source}")
set(calls CreateFileW:3 ReadFile:2 WriteFile:3 CloseHandle:2
    SetFilePointerEx:12 GetFileInformationByHandle:3 GetFileAttributesW:3
    GetFileAttributesExW:2 GetFinalPathNameByHandleW:3 SetEndOfFile:5
    FlushFileBuffers:1 FindFirstFileW:1 FindNextFileW:2 FindClose:4
    CreateDirectoryW:1 DeleteFileW:1 RemoveDirectoryW:1 MoveFileExW:1
    SetFileInformationByHandle:3)
foreach(rule IN LISTS calls)
    string(REPLACE ":" ";" pair "${rule}")
    list(GET pair 0 api)
    list(GET pair 1 count)
    string(REGEX MATCHALL "(^|[^A-Za-z0-9_])${api}\\(" matches "${source}")
    list(LENGTH matches actual)
    if(NOT actual EQUAL count)
        message(FATAL_ERROR "Pinned ${api} inventory changed: ${actual}, expected ${count}")
    endif()
    string(REGEX REPLACE "(^|[^A-Za-z0-9_])${api}\\(" "\\1gpw_${api}(" adapted "${adapted}")
endforeach()
# Declarations are included after the original kernel header has selected its
# Windows types. Explicit call substitutions avoid global header/API macros.
file(TO_CMAKE_PATH "${CMAKE_CURRENT_LIST_DIR}/windows_file.h" bridge_header)
string(REPLACE "#include \"kernel.h\""
    "#include \"kernel.h\"\n#if defined(_WIN32)\n#include \"${bridge_header}\"\n#endif"
    adapted "${adapted}")
file(WRITE "${OUTPUT}.tmp" "${adapted}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
