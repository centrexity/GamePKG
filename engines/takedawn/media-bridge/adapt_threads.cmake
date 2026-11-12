# Convert guest KTHREAD objects to owned host handles for priority operations.
# Refuse unknown upstream revisions and preserve the original source.
if(NOT DEFINED INPUT OR NOT DEFINED OUTPUT)
    message(FATAL_ERROR "Set INPUT and OUTPUT for the pinned thread adaptation")
endif()
get_filename_component(input_real "${INPUT}" REALPATH)
get_filename_component(output_real "${OUTPUT}" REALPATH)
if(input_real STREQUAL output_real)
    message(FATAL_ERROR "Never overwrite the pinned upstream thread source")
endif()
file(SHA256 "${INPUT}" hash)
if(NOT hash STREQUAL "bf62f8787a2e78a0abdb28e10ad8f79c5267dc655491353af231067747ce446e")
    message(FATAL_ERROR "Unreviewed xboxrecomp kernel_bridge.c; pinned hash does not match")
endif()
file(READ "${INPUT}" source)
set(before [==[static void bridge_KeQueryBasePriorityThread(void)
{
    g_eax = (uint32_t)xbox_KeQueryBasePriorityThread(
        XBOX_TO_NATIVE(STACK_ARG(0)));
}

static void bridge_KeSetBasePriorityThread(void)
{
    g_eax = (uint32_t)xbox_KeSetBasePriorityThread(
        XBOX_TO_NATIVE(STACK_ARG(0)), (LONG)STACK_ARG(1));
}

]==])
set(after [==[/* Ke* takes a guest KTHREAD object, never a host HANDLE. The current
 * object may be the initial TIB's KTHREAD or a spawned thread object.
 * Other live objects use an owned duplicate of the shadow handle so a
 * concurrent close cannot invalidate it during the priority operation. */
static HANDLE bridge_priority_thread(uint32_t va, int *owned);

static void bridge_KeQueryBasePriorityThread(void)
{
    int owned = 0;
    HANDLE thread = bridge_priority_thread(STACK_ARG(0), &owned);
    g_eax = thread ? (uint32_t)xbox_KeQueryBasePriorityThread(thread) : 0;
    if (owned) CloseHandle(thread);
}

static void bridge_KeSetBasePriorityThread(void)
{
    int owned = 0;
    HANDLE thread = bridge_priority_thread(STACK_ARG(0), &owned);
    g_eax = thread ? (uint32_t)xbox_KeSetBasePriorityThread(
        thread, (LONG)STACK_ARG(1)) : 0;
    if (owned) CloseHandle(thread);
}

]==])
string(REPLACE "${before}" "${after}" adapted "${source}")
if(adapted STREQUAL source)
    message(FATAL_ERROR "Pinned thread-priority wrappers are missing")
endif()
set(anchor "/* Remove (and close) the mapping for a guest object VA. */")
file(READ "${CMAKE_CURRENT_LIST_DIR}/thread_priority.inc.h" helper)
string(REPLACE "${anchor}" "${helper}\n${anchor}" with_helper "${adapted}")
if(with_helper STREQUAL adapted)
    message(FATAL_ERROR "Pinned KE shadow table anchor is missing")
endif()
# Insert the host-only CPU policy after checking the unchanged upstream input.
# Linux retains the same scheduler; the policy body and call are Windows-only.
file(READ "${CMAKE_CURRENT_LIST_DIR}/../rust-port/native_windows_cpu.inc.h" cpu_policy)
set(role_anchor "void xbox_log_thread_role(const char *role, uint32_t routine)\n{\n")
set(role_policy "${role_anchor}#if defined(_WIN32)\n    if (role && !strcmp(role, \"nv2a-ack\"))\n        gamepkg_windows_pin_executor();\n#endif\n")
string(REPLACE "${role_anchor}" "${role_policy}" with_role "${with_helper}")
if(with_role STREQUAL with_helper)
    message(FATAL_ERROR "Pinned executor thread role is missing")
endif()
set(include_anchor "#include \"kernel.h\"\n")
string(REPLACE "${include_anchor}" "${include_anchor}#if defined(_WIN32)\n${cpu_policy}\n#endif\n" with_cpu "${with_role}")
if(with_cpu STREQUAL with_role)
    message(FATAL_ERROR "Pinned kernel include is missing")
endif()
file(WRITE "${OUTPUT}.tmp" "/* GamePKG: resolve guest thread objects before host priority calls. */\n${with_cpu}")
execute_process(COMMAND "${CMAKE_COMMAND}" -E copy_if_different "${OUTPUT}.tmp" "${OUTPUT}" COMMAND_ERROR_IS_FATAL ANY)
file(REMOVE "${OUTPUT}.tmp")
