/* Included only by the reviewed glyph adapter. Immutable pictures after init;
 * renderer/overlay workers may read them concurrently without input locks. */
#include "prompt_art.h"
#include <stdlib.h>
static uint32_t *gp_art_pixels[30][4][4];
static int gp_art_loaded;
static atomic_int gp_art_ready;
static int gp_rle_decode(const struct gamepkg_prompt_rle *image, uint32_t *out, size_t count)
{
    if (!image || !image->runs || !out || !count || count > 512u * 256u
        || (size_t)image->width * image->height != count || !image->count || image->count > count)
        return 0;
    size_t at = 0;
    for (uint32_t i = 0; i < image->count; i++) {
        uint32_t n = image->runs[2u * i], value = image->runs[2u * i + 1];
        if (!n || n > count - at) return 0;
        for (uint32_t j = 0; j < n; j++) out[at++] = value;
    }
    return at == count;
}
static int gp_art_load(void)
{
    struct { const struct gamepkg_prompt_rle *source; uint32_t *pixels; } pool[480];
    unsigned used = 0;
    size_t bytes = 0;
    static const unsigned widths[] = {128,32,32,32}, heights[] = {64,32,32,16};
    for (unsigned v = 0; v < 30; v++) for (unsigned t = 0; t < 4; t++) for (unsigned f = 0; f < 4; f++) {
        const struct gamepkg_prompt_rle *source = gamepkg_kenney_art[v][t][f];
        if (!source) continue;
        if (source->width != widths[t] * (f + 1u) || source->height != heights[t] * (f + 1u)) goto fail;
        unsigned i;
        for (i = 0; i < used && pool[i].source != source; i++) {}
        if (i == used) {
            size_t count = (size_t)source->width * source->height;
            if (bytes + count * sizeof(uint32_t) > 32u * 1024u * 1024u) goto fail;
            uint32_t *pixels = (uint32_t *)malloc(count * sizeof(uint32_t));
            if (!pixels) goto fail;
            if (!gp_rle_decode(source, pixels, count)) { free(pixels); goto fail; }
            pool[used].source = source; pool[used++].pixels = pixels;
            bytes += count * sizeof(uint32_t);
        }
        gp_art_pixels[v][t][f] = pool[i].pixels;
    }
    fprintf(stderr, "[GamePKG prompts] Kenney CC0 vector artwork ready, %u images, %zu bytes\n", used, bytes);
    return 1;
fail:
    for (unsigned i = 0; i < used; i++) free(pool[i].pixels);
    memset(gp_art_pixels, 0, sizeof gp_art_pixels);
    fprintf(stderr, "[GamePKG prompts] artwork unavailable; retaining existing prompts\n");
    return 0;
}
