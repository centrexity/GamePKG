/* 4x Super-xBR to 3x in linear premultiplied space. Each destination
 * interval covers 4/3 source texels; integer overlap weights sum to 16.
 * Existing 2x/4x paths are untouched. Pic and CH come from the pinned scaler. */
static int gamepkg_pic_three(const Pic *src, Pic *dst)
{
    if (!src || !src->p || src->w <= 0 || src->h <= 0 || src->w % 4 || src->h % 4
        || !pic_new(dst, src->w / 4 * 3, src->h / 4 * 3)) return 0;
    for (int y = 0; y < dst->h; y++) for (int x = 0; x < dst->w; x++) {
        float *out = dst->p + ((size_t)y * dst->w + x) * CH;
        for (int k = 0; k < CH; k++) out[k] = 0;
        int left = x * 4, top = y * 4;
        for (int sy = top / 3; sy <= (top + 3) / 3; sy++) {
            int low_y = top > sy * 3 ? top : sy * 3;
            int high_y = top + 4 < (sy + 1) * 3 ? top + 4 : (sy + 1) * 3;
            for (int sx = left / 3; sx <= (left + 3) / 3; sx++) {
                int low_x = left > sx * 3 ? left : sx * 3;
                int high_x = left + 4 < (sx + 1) * 3 ? left + 4 : (sx + 1) * 3;
                float weight = (float)((high_y - low_y) * (high_x - low_x)) / 16.0f;
                const float *in = src->p + ((size_t)sy * src->w + sx) * CH;
                float alpha = in[3] < 0 ? 0 : in[3] > 1 ? 1 : in[3];
                for (int k = 0; k < CH; k++) {
                    float value = in[k];
                    if (k < 3) value = value < 0 ? 0 : value > alpha ? alpha : value;
                    else if (k == 3) value = alpha;
                    out[k] += value * weight;
                }
            }
        }
    }
    return 1;
}
