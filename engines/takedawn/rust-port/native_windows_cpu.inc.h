/* Host-reviewed Windows graphics-worker placement. No guest state is changed. */
#ifndef GAMEPKG_NATIVE_WINDOWS_CPU_H
#define GAMEPKG_NATIVE_WINDOWS_CPU_H
#include <stdint.h>
#include <stddef.h>
#include <string.h>

typedef struct {
    uint16_t group;
    uint8_t logical, core, efficiency, flags;
} GamepkgHostCpu;

static int gamepkg_cpu_allowed(const GamepkgHostCpu *cpu, uint64_t affinity)
{
    return cpu->group == 0 && cpu->logical < 64
        && (affinity & (UINT64_C(1) << cpu->logical))
        && (!(cpu->flags & 2) || (cpu->flags & 4));
}

/* Only hybrid CPUs need this policy. Keep the worker off the guest's whole
 * physical core, including its SMT sibling. Missing/ambiguous topology leaves
 * Windows in charge. The lowest eligible fast logical CPU is deterministic. */
static int gamepkg_executor_cpu(const GamepkgHostCpu *cpus, size_t count,
                               uint64_t affinity, uint64_t guest)
{
    unsigned low = 255, high = 0;
    int guest_core = -1, selected = -1;
    size_t i;
    if (!guest || (guest & (guest - 1)) || !(guest & affinity))
        return -1;
    for (i = 0; i < count; i++) {
        const GamepkgHostCpu *cpu = &cpus[i];
        size_t j;
        if (!gamepkg_cpu_allowed(cpu, affinity))
            continue;
        for (j = 0; j < i; j++)
            if (gamepkg_cpu_allowed(&cpus[j], affinity) && cpus[j].logical == cpu->logical)
                return -1;
        if (cpu->efficiency < low) low = cpu->efficiency;
        if (cpu->efficiency > high) high = cpu->efficiency;
        if (guest == (UINT64_C(1) << cpu->logical)) {
            if (guest_core >= 0) return -1;
            guest_core = cpu->core;
        }
    }
    if (guest_core < 0 || low >= high)
        return -1;
    for (i = 0; i < count; i++) {
        const GamepkgHostCpu *cpu = &cpus[i];
        if (gamepkg_cpu_allowed(cpu, affinity) && cpu->core != guest_core
                && cpu->efficiency == high
                && (selected < 0 || cpu->logical < selected))
            selected = cpu->logical;
    }
    return selected;
}

/* SYSTEM_CPU_SET_INFORMATION is a sized sequence. Read the documented stable
 * fields with memcpy, without alignment assumptions or newer SDK headers.
 * Reject truncation/overflow rather than selecting from partial information. */
static int gamepkg_decode_cpu_sets(const unsigned char *bytes, size_t size,
                                  GamepkgHostCpu *out, size_t capacity)
{
    size_t offset = 0, count = 0;
    while (offset < size) {
        uint32_t length, type;
        uint16_t group;
        if (size - offset < 8) return -1;
        memcpy(&length, bytes + offset, 4);
        memcpy(&type, bytes + offset + 4, 4);
        if (length < 8 || length > size - offset) return -1;
        if (type == 0) {
            if (length < 32) return -1;
            memcpy(&group, bytes + offset + 12, 2);
            if (group == 0) {
                if (count >= capacity) return -1;
                out[count].group = group;
                out[count].logical = bytes[offset + 14];
                out[count].core = bytes[offset + 15];
                out[count].efficiency = bytes[offset + 18];
                out[count].flags = bytes[offset + 19];
                count++;
            }
        }
        offset += length;
    }
    return (int)count;
}

#if defined(_WIN32)
#include <windows.h>
#include <stdlib.h>
#include <stdio.h>

static void gamepkg_windows_pin_executor(void)
{
    typedef BOOL (WINAPI *QueryCpuSets)(void *, ULONG, ULONG *, HANDLE, ULONG);
    QueryCpuSets query;
    HMODULE kernel;
    GROUP_AFFINITY group;
    DWORD_PTR affinity = 0, system = 0;
    ULONG needed = 0, returned;
    unsigned char *bytes;
    GamepkgHostCpu cpus[64];
    uint64_t guest = (uint64_t)xbox_GuestCpuMask();
    int count, cpu;
    const char *disabled = getenv("GAMEPKG_WINDOWS_EXECUTOR_CPU");
    if ((disabled && !strcmp(disabled, "off")) || !guest)
        return;
    memset(&group, 0, sizeof group);
    if (!GetThreadGroupAffinity(GetCurrentThread(), &group) || group.Group != 0
            || !GetProcessAffinityMask(GetCurrentProcess(), &affinity, &system))
        return;
    kernel = GetModuleHandleW(L"kernel32.dll");
    query = kernel ? (QueryCpuSets)(void *)GetProcAddress(kernel, "GetSystemCpuSetInformation") : NULL;
    if (!query) return; /* Older Windows: retain existing scheduling. */
    if (query(NULL, 0, &needed, GetCurrentProcess(), 0)
            || GetLastError() != ERROR_INSUFFICIENT_BUFFER || !needed || needed > 1048576)
        return;
    bytes = (unsigned char *)malloc(needed);
    if (!bytes) return;
    returned = needed;
    if (!query(bytes, needed, &returned, GetCurrentProcess(), 0) || returned > needed) {
        free(bytes);
        return;
    }
    count = gamepkg_decode_cpu_sets(bytes, returned, cpus, 64);
    free(bytes);
    if (count < 0) return;
    cpu = gamepkg_executor_cpu(cpus, (size_t)count, (uint64_t)affinity, guest);
    if (cpu >= 0 && SetThreadAffinityMask(GetCurrentThread(), (DWORD_PTR)1 << cpu))
        fprintf(stderr, "[GamePKG] graphics worker on fast CPU %d, separate from guest core\n", cpu);
}
#endif
#endif
