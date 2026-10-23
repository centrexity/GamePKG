/* Independent ideal affine-nearest oracle for the pinned synthetic checker
 * scene, not a console-fidelity oracle. No game media or renderer code is used.
 * Input: the two exact 640x480 BGR24 BMPs written by the smoke fixture. */
#include <stdio.h>
#include <stdint.h>
#include <string.h>

static uint8_t images[2][640 * 480 * 3];
static uint32_t little32(const uint8_t *p) {
    return (uint32_t)p[0] | (uint32_t)p[1] << 8 | (uint32_t)p[2] << 16 | (uint32_t)p[3] << 24;
}
static int load(const char *path, int image) {
    uint8_t h[54];
    FILE *file = fopen(path, "rb");
    if (!file) { perror(path); return 0; }
    int ok = fread(h, 1, sizeof h, file) == sizeof h
        && !memcmp(h, "BM", 2) && little32(h + 2) == 54 + sizeof images[image]
        && little32(h + 10) == 54 && little32(h + 14) == 40
        && little32(h + 18) == 640 && little32(h + 22) == 480
        && h[26] == 1 && h[27] == 0 && h[28] == 24 && h[29] == 0 && little32(h + 30) == 0
        && fread(images[image], 1, sizeof images[image], file) == sizeof images[image]
        && fgetc(file) == EOF && !ferror(file);
    fclose(file);
    if (!ok) fprintf(stderr, "%s: expected the pinned 640x480 BGR24 smoke BMP\n", path);
    return ok;
}
static uint32_t pixel(unsigned image, unsigned x, unsigned y) {
    const uint8_t *p = &images[image][((479 - y) * 640 + x) * 3];
    return (uint32_t)p[0] | (uint32_t)p[1] << 8 | (uint32_t)p[2] << 16;
}
int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: %s checker_cpu.bmp checker_gpu.bmp\n", argv[0]); return 2; }
    if (!load(argv[1], 0) || !load(argv[2], 1)) return 2;
    unsigned different = 0, wrong[2] = {0, 0}, columns[640] = {0}, rows[480] = {0};
    for (unsigned y = 40; y < 220; y++) for (unsigned x = 440; x < 620; x++) {
        /* Four screen-space corners: (440,40)..(620,220), UV0..2.
         * A 64x64 repeat texture, sample centers x+.5/y+.5. Integer rational
         * arithmetic makes exact texel boundaries independent of host FP. */
        unsigned u = ((2 * (x - 440) + 1) * 64 / 180) % 64;
        unsigned v = ((2 * (y - 40) + 1) * 64 / 180) % 64;
        uint32_t want = (((u >> 3) ^ (v >> 3)) & 1) ? 0x2040FFu : 0xFF8000u;
        if (v == 0) want = 0xFFFFFFu;
        if (u == 0) want = 0x00FF00u;
        uint32_t cpu = pixel(0, x, y), gpu = pixel(1, x, y);
        wrong[0] += cpu != want;
        wrong[1] += gpu != want;
        if (cpu != gpu) {
            if (different < 12)
                printf("(%u,%u) texel(%u,%u) cpu=%06X gpu=%06X ideal=%06X\n", x, y, u, v, cpu, gpu, want);
            different++; columns[x]++; rows[y]++;
        }
    }
    printf("different=%u; ideal affine-nearest mismatches: cpu=%u gpu=%u\n", different, wrong[0], wrong[1]);
    for (unsigned x = 440; x < 620; x++) if (columns[x] > 100)
        printf("column %u: %u different pixels\n", x, columns[x]);
    for (unsigned y = 40; y < 220; y++) if (rows[y] > 100)
        printf("row %u: %u different pixels\n", y, rows[y]);
    return wrong[0] || wrong[1] ? 1 : 0;
}
