/* Included after the pinned glyphs.c tables. Host graphics adaptation only:
 * game bodies, input delivery and original artwork remain unchanged. */
#include <stdatomic.h>
#include <string.h>

enum { GP_GLYPH_SETS = 15, GP_GLYPH_VARIANTS = 29 };
_Static_assert((int)XBOX_GLYPH_SET_COUNT == GP_GLYPH_SETS, "review the glyph families");
static uint32_t gp_sheets[GP_GLYPH_SETS][128 * 64];
static uint32_t gp_a[GP_GLYPH_SETS][32 * 32];
static uint32_t gp_b[GP_GLYPH_SETS][32 * 32];
static struct nv2a_texrep_variant gp_variants[4][GP_GLYPH_VARIANTS];
static struct nv2a_texrep_tex gp_textures[4];
static atomic_flag gp_glyph_lock = ATOMIC_FLAG_INIT;
static unsigned gp_ps2_ports;
static int gp_base_set;

static void gp_lock(void)
{
    while (atomic_flag_test_and_set_explicit(&gp_glyph_lock, memory_order_acquire)) {}
}
static void gp_unlock(void)
{
    atomic_flag_clear_explicit(&gp_glyph_lock, memory_order_release);
}
/* No input or renderer locks are acquired here. The upload-generation API is
 * atomic, and tables/images are immutable after registration. */
static void gp_publish(void)
{
    int ps2 = gp_ps2_ports != 0 && gp_base_set != XBOX_GLYPH_SET_KEYBOARD;
    nv2a_texrep_set_variant(ps2 ? GP_GLYPH_SETS + gp_base_set : gp_base_set);
}
void gamepkg_glyphs_scene(int ps2_driving)
{
    gp_lock();
    if (!ps2_driving) gp_ps2_ports = 0;
    gp_publish();
    gp_unlock();
}
void gamepkg_glyphs_port(unsigned port, int mapped_ps2)
{
    if (port >= 4) return;
    gp_lock();
    if (mapped_ps2) gp_ps2_ports |= 1u << port;
    else gp_ps2_ports &= ~(1u << port);
    gp_publish();
    gp_unlock();
}
static void gamepkg_glyphs_base(int set)
{
    gp_lock();
    gp_base_set = set >= 0 && set < GP_GLYPH_SETS ? set : 0;
    gp_publish();
    gp_unlock();
}

struct gp_rect { unsigned x, y, w, h; };
enum { GP_B, GP_A, GP_X, GP_Y, GP_WHITE, GP_BLACK, GP_LT, GP_RT };
static const struct gp_rect gp_cells[] = {
    {0, 0, 30, 30}, {30, 0, 30, 30}, {60, 0, 30, 30}, {90, 0, 30, 30},
    {1, 37, 28, 19}, {31, 37, 28, 19}, {62, 29, 18, 18}, {62, 46, 18, 18}
};
static unsigned gp_face_source(unsigned button, int swapped)
{
    if (!swapped) return button;
    if (button == GP_A) return GP_B;
    if (button == GP_B) return GP_A;
    if (button == GP_X) return GP_Y;
    if (button == GP_Y) return GP_X;
    return button;
}
static void gp_clear(uint32_t *image, struct gp_rect r)
{
    for (unsigned y = 0; y < r.h; y++)
        memset(image + (r.y + y) * 128 + r.x, 0, r.w * sizeof *image);
}
/* Fit one physical glyph into a logical button's existing cell, preserving its
 * aspect ratio. Source/destination are separate immutable/initialization images. */
static void gp_copy(uint32_t *dst, struct gp_rect d, const uint32_t *src, struct gp_rect s)
{
    unsigned w = d.w, h = s.h * w / s.w;
    if (h > d.h) { h = d.h; w = s.w * h / s.h; }
    gp_clear(dst, d);
    unsigned x0 = d.x + (d.w - w) / 2, y0 = d.y + (d.h - h) / 2;
    for (unsigned y = 0; y < h; y++)
        for (unsigned x = 0; x < w; x++)
            dst[(y0 + y) * 128 + x0 + x] =
                src[(s.y + y * s.h / h) * 128 + s.x + x * s.w / w];
}
static void gp_single(uint32_t *dst, const uint32_t *sheet, unsigned button)
{
    memset(dst, 0, 32 * 32 * sizeof *dst);
    struct gp_rect r = gp_cells[button];
    for (unsigned y = 0; y < 30; y++)
        memcpy(dst + (y + 1) * 32 + 1, sheet + (r.y + y) * 128 + r.x,
               30 * sizeof *dst);
}
static void gp_transform_sheet(uint32_t *logical, const uint32_t *physical, int swapped)
{
    memcpy(logical, physical, 128 * 64 * sizeof *logical);
    gp_clear(logical, (struct gp_rect){0, 0, 128, 30});
    gp_clear(logical, (struct gp_rect){0, 30, 62, 34});
    gp_clear(logical, (struct gp_rect){62, 29, 18, 35});
    /* Match controls.h: physical R2->B, R1->A, L1->X, Triangle->Y,
     * Square->LT, Cross->RT, L2->Black. White has no PS2 driving action. */
    static const unsigned from[] = {
        GP_RT, GP_BLACK, GP_WHITE, GP_Y, GP_WHITE, GP_LT, GP_X, GP_A
    };
    for (unsigned button = 0; button < 8; button++) {
        if (button == GP_WHITE) gp_clear(logical, gp_cells[button]);
        else gp_copy(logical, gp_cells[button], physical,
                     gp_cells[gp_face_source(from[button], swapped)]);
    }
}
static void gp_make_sheet(unsigned set)
{
    unsigned source_set = set;
    /* Original/modern Xbox normally keep the retail face pixels, so their
     * host sheet has no complete face row. Its drawn swapped set supplies all
     * four faces and shoulders; undo that face swap when selecting positions. */
    if (set == 0 || set == XBOX_GLYPH_SET_XBOX_MODERN || set == XBOX_GLYPH_SET_XBOX_SWAPPED)
        source_set = XBOX_GLYPH_SET_XBOX_MODERN_SWAPPED;
    const uint32_t *physical = k_sheet[source_set - 1].image;
    int swapped = source_set == XBOX_GLYPH_SET_NINTENDO_LABEL
        || source_set == XBOX_GLYPH_SET_POSITIONAL_SWAPPED
        || source_set == XBOX_GLYPH_SET_XBOX_SWAPPED
        || source_set == XBOX_GLYPH_SET_PLAYSTATION_SWAPPED
        || source_set == XBOX_GLYPH_SET_PLAYSTATION5_SWAPPED
        || source_set == XBOX_GLYPH_SET_STEAM_SWAPPED
        || source_set == XBOX_GLYPH_SET_XBOX_MODERN_SWAPPED;
    uint32_t *logical = gp_sheets[set];
    gp_transform_sheet(logical, physical, swapped);
    gp_single(gp_a[set], logical, GP_A);
    gp_single(gp_b[set], logical, GP_B);
}
static void gamepkg_glyphs_register(void)
{
    _Static_assert(sizeof k_tex / sizeof k_tex[0] == 4, "review prompt textures");
    for (unsigned tex = 0; tex < 4; tex++) {
        gp_textures[tex] = k_tex[tex];
        memcpy(gp_variants[tex], k_tex[tex].variants,
               (GP_GLYPH_SETS - 1) * sizeof gp_variants[tex][0]);
        gp_textures[tex].variants = gp_variants[tex];
        gp_textures[tex].n_variants = GP_GLYPH_VARIANTS;
        for (unsigned set = 0; set < GP_GLYPH_SETS; set++)
            if (set) gp_variants[tex][GP_GLYPH_SETS - 1 + set] = k_tex[tex].variants[set - 1];
    }
    for (unsigned set = 0; set < GP_GLYPH_SETS; set++) {
        gp_make_sheet(set);
        unsigned index = GP_GLYPH_SETS - 1 + set;
        gp_variants[0][index] = (struct nv2a_texrep_variant){gp_sheets[set], k_sheet_all, 1, 0, 0};
        gp_variants[1][index] = (struct nv2a_texrep_variant){gp_a[set], k_single, 1, 0, 0};
        gp_variants[2][index] = (struct nv2a_texrep_variant){gp_b[set], k_single, 1, 0, 0};
    }
    nv2a_texrep_register(gp_textures, 4);
}
