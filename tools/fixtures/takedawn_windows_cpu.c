#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include "../../engines/takedawn/rust-port/native_windows_cpu.inc.h"

static uint32_t seed = 1729;
static uint32_t random_word(void) { seed = seed * 1664525u + 1013904223u; return seed; }
int main(void)
{
    GamepkgHostCpu hybrid[] = {
        {0,0,0,1,0},{0,1,0,1,0},{0,2,2,1,0},{0,3,2,1,0},
        {0,4,4,0,0},{0,5,5,0,0},{0,6,6,0,0},{0,7,7,0,0},
        {0,8,8,0,0},{0,9,9,0,0},{0,10,10,0,0},{0,11,11,0,0}
    };
    GamepkgHostCpu cpus[64], decoded[64];
    unsigned char bytes[2049];
    size_t i, n;
    assert(gamepkg_executor_cpu(hybrid, 12, 4095, 4) == 0);
    assert(gamepkg_executor_cpu(hybrid, 12, 4094, 4) == 1);
    assert(gamepkg_executor_cpu(hybrid, 12, 4095, 0) == -1);
    assert(gamepkg_executor_cpu(hybrid, 12, 4095, 3) == -1);
    assert(gamepkg_executor_cpu(hybrid, 12, 4091, 4) == -1);
    assert(gamepkg_executor_cpu(hybrid, 12, 4080, 16) == -1);
    assert(gamepkg_executor_cpu(hybrid, 4, 15, 4) == -1);
    assert(gamepkg_executor_cpu(hybrid, 2, 3, 1) == -1);
    memcpy(cpus, hybrid, sizeof hybrid);
    cpus[5].logical = cpus[0].logical;
    assert(gamepkg_executor_cpu(cpus, 12, 4095, 4) == -1);
    memcpy(cpus, hybrid, sizeof hybrid);
    cpus[0].flags = cpus[1].flags = 2; /* Allocated to another process. */
    assert(gamepkg_executor_cpu(cpus, 12, 4095, 4) == -1);
    cpus[1].flags = 6; /* Allocated to this process. */
    assert(gamepkg_executor_cpu(cpus, 12, 4095, 4) == 1);
    memcpy(cpus, hybrid, sizeof hybrid);
    cpus[0].group = cpus[1].group = 1;
    assert(gamepkg_executor_cpu(cpus, 12, 4095, 4) == -1);
    for (i = 0; i < 12; i++) cpus[i].efficiency = 0;
    assert(gamepkg_executor_cpu(cpus, 12, 4095, 4) == -1);
    memset(bytes, 0, sizeof bytes);
    for (i = 0; i < 64; i++) {
        uint32_t length = 32;
        memcpy(bytes + i * 32, &length, 4);
        bytes[i * 32 + 14] = (unsigned char)i;
        bytes[i * 32 + 15] = (unsigned char)(i / 2);
        bytes[i * 32 + 18] = (unsigned char)(i < 4);
    }
    assert(gamepkg_decode_cpu_sets(bytes, 2048, decoded, 64) == 64);
    assert(gamepkg_executor_cpu(decoded, 64, UINT64_MAX, 4) == 0);
    for (n = 1; n < 2048; n++)
        assert(gamepkg_decode_cpu_sets(bytes, n, decoded, 64) == (n % 32 ? -1 : (int)(n / 32)));
    assert(gamepkg_decode_cpu_sets(bytes, 2049, decoded, 64) == -1);
    assert(gamepkg_decode_cpu_sets(bytes, 2048, decoded, 63) == -1);
    /* Unaligned input keeps the stable fields. */
    memmove(bytes + 1, bytes, 2048);
    assert(gamepkg_decode_cpu_sets(bytes + 1, 2048, decoded, 64) == 64);
    {
        uint32_t length = 40, type = 7;
        memset(bytes, 0, 80);
        memcpy(bytes, &length, 4);
        memcpy(bytes + 40, &length, 4);
        memcpy(bytes + 44, &type, 4); /* Unknown future type is skipped. */
        bytes[14] = 2; bytes[15] = 2; bytes[18] = 1;
        assert(gamepkg_decode_cpu_sets(bytes, 80, decoded, 64) == 1);
        assert(decoded[0].logical == 2 && decoded[0].efficiency == 1);
    }
    for (n = 0; n < 10000; n++) {
        int picked;
        uint64_t affinity = ((uint64_t)random_word() << 32) | random_word();
        unsigned guest = random_word() % 64;
        for (i = 0; i < 64; i++) {
            cpus[i].group = 0; cpus[i].logical = (uint8_t)i;
            cpus[i].core = (uint8_t)(i / 2);
            cpus[i].efficiency = (uint8_t)(random_word() % 3);
            cpus[i].flags = (uint8_t)(random_word() % 8);
        }
        picked = gamepkg_executor_cpu(cpus, 64, affinity, UINT64_C(1) << guest);
        if (picked >= 0) {
            assert(picked < 64 && (affinity & (UINT64_C(1) << picked)));
            assert(cpus[picked].core != cpus[guest].core);
            assert(!(cpus[picked].flags & 2) || (cpus[picked].flags & 4));
            for (i = 0; i < 64; i++) {
                if ((affinity & (UINT64_C(1) << i)) && (!(cpus[i].flags & 2) || (cpus[i].flags & 4)))
                    assert(cpus[picked].efficiency >= cpus[i].efficiency);
            }
        }
        for (i = 0; i < 257; i++) bytes[i] = (unsigned char)random_word();
        (void)gamepkg_decode_cpu_sets(bytes, random_word() % 257, decoded, 64);
    }
    puts("PASS: hybrid placement, guest physical-core separation, affinity and allocation restrictions, homogeneous fallback, truncated/unaligned CPU sets, 10000 randomized cases");
    return 0;
}
