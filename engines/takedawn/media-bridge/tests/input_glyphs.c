/* Asset-free host-UI gate using the actual staged glyph source and owned art.
 * No retail textures, executable or ROM are supplied to this process. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdatomic.h>
#include "nv2a_texrep.h"
#include "input/pad_glyphs.h"
#include "controls.h"
static const struct nv2a_texrep_tex *registered;
static unsigned registered_count;
static _Atomic int selected;
static void (*listener)(int);
void nv2a_texrep_register(const struct nv2a_texrep_tex *textures, unsigned count)
{
    registered = textures; registered_count = count;
}
void nv2a_texrep_set_variant(int value) { atomic_store(&selected, value); }
void xbox_InputSetGlyphListener(void (*callback)(int)) { listener = callback; callback(0); }
#include GAMEPKG_GLYPHS_SOURCE

/* Independently ask the original input mapper what each physical control does,
 * then check that the corresponding rendered cell identifies that control.
 * Uniform colored glyphs make this independent of font/antialiasing details. */
static void input_render_agree(void)
{
    static const unsigned cell_of_pad[] = {
        GP_A, GP_B, GP_X, GP_Y, GP_BLACK, GP_WHITE, GP_LT, GP_RT
    };
    for (int mirrored = 0; mirrored <= 1; mirrored++) {
        uint32_t physical[128 * 64] = {0}, logical[128 * 64];
        for (unsigned button = 0; button < 8; button++) {
            unsigned where = cell_of_pad[button];
            if (mirrored) {
                if (where == GP_A) where = GP_B;
                else if (where == GP_B) where = GP_A;
                else if (where == GP_X) where = GP_Y;
                else if (where == GP_Y) where = GP_X;
            }
            struct gp_rect r = gp_cells[where];
            for (unsigned y = 0; y < r.h; y++)
                for (unsigned x = 0; x < r.w; x++)
                    physical[(r.y + y) * 128 + r.x + x] = 0xff000001u + button;
        }
        gp_transform_sheet(logical, physical, mirrored);
        for (unsigned button = 0; button < 8; button++) {
            XBOX_GAMEPAD state = {0};
            state.bAnalogButtons[button] = 255;
            if (mirrored) bo3_ctl_mirror(&state);
            bo3_ctl_ps2(&state, mirrored);
            for (unsigned guest = 0; guest < 8; guest++) if (state.bAnalogButtons[guest]) {
                struct gp_rect r = gp_cells[cell_of_pad[guest]];
                assert(logical[(r.y + r.h / 2) * 128 + r.x + r.w / 2] == 0xff000001u + button);
            }
        }
        struct gp_rect unused = gp_cells[GP_WHITE];
        assert(logical[(unused.y + unused.h / 2) * 128 + unused.x + unused.w / 2] == 0);
    }
}

static unsigned colored(const uint32_t *pixels, unsigned count)
{
    unsigned out = 0;
    for (unsigned i = 0; i < count; i++) out += pixels[i] >> 24 != 0;
    return out;
}
static void same_variant(const struct nv2a_texrep_variant *a,
                         const struct nv2a_texrep_variant *b)
{
    assert(a->image == b->image && a->rects == b->rects && a->n_rects == b->n_rects);
    assert(a->keep_alpha == b->keep_alpha && a->stage0_only == b->stage0_only);
}
int main(int argc, char **argv)
{
    assert(argc == 2);
    input_render_agree();
    bo3_glyphs_init();
    assert(registered_count == 4 && listener && atomic_load(&selected) == 30);
    unsigned checks = 0;
    for (int set = 0; set < GP_GLYPH_SETS; set++) {
        gamepkg_glyphs_scene(0);
        listener(set);
        assert(atomic_load(&selected) == gp_normal_variant(set));
        for (unsigned texture = 0; texture < 4; texture++) {
            assert(registered[texture].hash == k_tex[texture].hash);
            assert(registered[texture].width == k_tex[texture].width);
            assert(registered[texture].height == k_tex[texture].height);
            if (set && !gp_art_pixels[set - 1][texture][0]) same_variant(&registered[texture].variants[set - 1], &k_tex[texture].variants[set - 1]);
        }
        gamepkg_glyphs_scene(1);
        gamepkg_glyphs_port(2, 1);
        int expected = set == XBOX_GLYPH_SET_KEYBOARD ? set : GP_GLYPH_SETS + set;
        assert(atomic_load(&selected) == expected);
        if (set != XBOX_GLYPH_SET_KEYBOARD) {
            const struct nv2a_texrep_variant *a = &registered[1].variants[expected - 1];
            const struct nv2a_texrep_variant *b = &registered[2].variants[expected - 1];
            assert(a->n_rects == 1 && b->n_rects == 1);
            assert(colored(a->image, 32 * 32) > 30);
            assert(colored(b->image, 32 * 32) > 30);
        }
        /* Disconnect and pause/results restore the configured base selection.
         * Another mapped controller keeps the shared glyph layer active. */
        gamepkg_glyphs_port(3, 1);
        gamepkg_glyphs_port(2, 0);
        assert(atomic_load(&selected) == expected);
        gamepkg_glyphs_port(3, 0);
        assert(atomic_load(&selected) == gp_normal_variant(set));
        gamepkg_glyphs_port(2, 1);
        gamepkg_glyphs_scene(0);
        assert(atomic_load(&selected) == gp_normal_variant(set));
        gamepkg_glyphs_port(99, 1);
        assert(atomic_load(&selected) == gp_normal_variant(set));
        checks++;
    }
    /* High-resolution pictures must use vector detail, touch only exact
     * prompt inputs and stay within the caller's output allocation. */
    unsigned vector_images = 0;
    for (unsigned v = 0; v < GP_GLYPH_VARIANTS; v++) for (unsigned t = 0; t < 4; t++) {
        const struct nv2a_texrep_variant *variant = &registered[t].variants[v];
        for (unsigned f = 1; f < 4; f++) {
            const uint32_t *high = gp_art_pixels[v][t][f];
            if (!high || !variant->image) continue;
            unsigned factor = f + 1u, w = registered[t].width, h = registered[t].height;
            size_t n = (size_t)w * h * factor * factor;
            uint32_t *out = malloc((n + 2) * sizeof *out); assert(out);
            out[0] = 0x12345678; out[n + 1] = 0x87654321;
            assert(gamepkg_prompt_scale(variant->image, w, h, factor, out + 1));
            assert(!memcmp(out + 1, high, n * sizeof *out));
            assert(out[0] == 0x12345678 && out[n + 1] == 0x87654321);
            unsigned differs = 0;
            for (unsigned y = 0; y < h * factor; y++) for (unsigned x = 0; x < w * factor; x++)
                differs += out[1 + y * w * factor + x] != variant->image[(y / factor) * w + x / factor];
            assert(differs > 10); /* actual vector detail, not enlarged low-res pixels */
            free(out); vector_images++;
        }
    }
    assert(vector_images >= 252);
    uint32_t unrelated[128 * 64]; memset(unrelated, 0x77, sizeof unrelated);
    uint32_t untouched = 0x12345678;
    assert(!gamepkg_prompt_scale(unrelated, 128, 64, 2, &untouched));
    assert(!gamepkg_prompt_scale(NULL, 128, 64, 2, &untouched));
    assert(!gamepkg_prompt_scale(unrelated, UINT32_MAX, 64, 2, &untouched));
    assert(!gamepkg_prompt_scale(unrelated, 128, 64, 8, &untouched));
    assert(untouched == 0x12345678);
    gp_art_loaded = 0; /* unavailable/OOM art must preserve the original overlay */
    assert(!gamepkg_prompt_sheet(XBOX_GLYPH_SET_XBOX_MODERN, &untouched));
    assert(untouched == 0x12345678);
    gp_art_loaded = 1;
    const uint32_t malformed_runs[] = { 5, 0xffabcdef };
    struct gamepkg_prompt_rle bad = { 2, 2, 1, malformed_runs };
    uint32_t small[6] = {1,2,3,4,5,6};
    assert(!gp_rle_decode(&bad, small + 1, 4));
    assert(small[0] == 1 && small[5] == 6);
    printf("%u high-resolution prompt variants verified with output guards and unrelated-image rejection\n", vector_images);
    /* Export the actual owned PS2 glyphs for visual inspection before a game
     * build. PPM is merely the fixture's output format, never a player tool. */
    listener(XBOX_GLYPH_SET_PLAYSTATION);
    gamepkg_glyphs_scene(1);
    gamepkg_glyphs_port(2, 1);
    FILE *out = fopen(argv[1], "wb"); assert(out);
    fprintf(out, "P6\n64 32\n255\n");
    for (unsigned y = 0; y < 32; y++) for (unsigned x = 0; x < 64; x++) {
        uint32_t pixel = (x < 32 ? gp_a[1] : gp_b[1])[y * 32 + x % 32];
        unsigned alpha = pixel >> 24;
        unsigned char rgb[3] = {
            (unsigned char)(((pixel >> 16) & 255) * alpha / 255),
            (unsigned char)(((pixel >> 8) & 255) * alpha / 255),
            (unsigned char)((pixel & 255) * alpha / 255)
        };
        assert(fwrite(rgb, 1, 3, out) == 3);
    }
    assert(fclose(out) == 0);
    printf("%u glyph families: button mapping preserved; Kenney art and PlayStation fallback checked; mapped controller, keyboard, disconnect, pause/results and shared-controller transitions passed\n", checks);
    return 0;
}
