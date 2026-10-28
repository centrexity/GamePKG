// Frozen pre-index raster algorithm; shares only the independently checked shaper.
use super::dynamic_font::unicode_shape;
use crate::dynamic_font_data;
fn unicode_word(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}
fn unicode_find(data: &[u8], stride: usize, key: u32) -> usize {
    let mut lower = 0;
    let mut upper = data.len() / stride;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if unicode_word(data, middle * stride) < key {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    lower * stride
}
fn unicode_pair(data: &[u8], first: u32, second: u32) -> usize {
    let mut lower = 0;
    let mut upper = data.len() / 12;
    let wanted = (u64::from(first) << 32) | u64::from(second);
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let key = (u64::from(unicode_word(data, middle * 12)) << 32)
            | u64::from(unicode_word(data, middle * 12 + 4));
        if key < wanted {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    let at = lower * 12;
    if at < data.len() && unicode_word(data, at) == first && unicode_word(data, at + 4) == second {
        at
    } else {
        data.len()
    }
}
fn unicode_is_mark(glyph_id: u32) -> bool {
    let data = dynamic_font_data::unicode_marks();
    let at = unicode_find(data, 8, glyph_id);
    at < data.len() && unicode_word(data, at) == glyph_id
}
fn unicode_zero_glyph(glyph_id: u32) -> bool {
    (glyph_id & 0xffff) as usize == dynamic_font_data::UNICODE_GLYPH_COUNT - 1
}
fn unicode_metrics(glyph_id: u32) -> [i32; 7] {
    let data = dynamic_font_data::unicode_atlas_metadata();
    let glyph_id = glyph_id & 0xffff;
    let index = if (glyph_id as usize) < dynamic_font_data::UNICODE_GLYPH_COUNT {
        glyph_id as usize
    } else {
        0
    };
    let at = index * 20;
    [
        i16::from_le_bytes([data[at], data[at + 1]]) as i32,
        i16::from_le_bytes([data[at + 2], data[at + 3]]) as i32,
        u16::from_le_bytes([data[at + 4], data[at + 5]]) as i32,
        u16::from_le_bytes([data[at + 6], data[at + 7]]) as i32,
        unicode_word(data, at + 8) as i32,
        unicode_word(data, at + 12) as i32,
        unicode_word(data, at + 16) as i32,
    ]
}
fn unicode_advance(points: &[u32], index: usize) -> i32 {
    let glyph_id = points[index];
    let mut glyph_advance = unicode_metrics(glyph_id)[4];
    if !unicode_is_mark(glyph_id) && !unicode_zero_glyph(glyph_id) {
        let mut next = index + 1;
        while next < points.len()
            && (unicode_is_mark(points[next])
                || unicode_zero_glyph(points[next]) && points[next] & 0x20000000 == 0)
        {
            next += 1;
        }
        if next < points.len() {
            let data = dynamic_font_data::unicode_kern();
            let at = unicode_pair(data, glyph_id, points[next]);
            if at < data.len() {
                glyph_advance += unicode_word(data, at + 8) as i32;
            }
        }
    }
    glyph_advance
}
pub struct DynamicLabel {
    points: Vec<u32>,
    pub width: i32,
    pub height: i32,
    origin_x: i32,
    origin_y: i32,
}
pub fn unicode_layout(content: &str) -> Option<DynamicLabel> {
    let points = unicode_shape(content)?;
    let mut x = 0i32;
    let mut min_x = 0;
    let mut max_x = 0;
    let mut min_y = 0;
    let mut max_y = dynamic_font_data::UNICODE_HEIGHT;
    for index in 0..points.len() {
        let metrics = unicode_metrics(points[index]);
        let left = x.div_euclid(64) + metrics[0];
        let top = dynamic_font_data::UNICODE_ASCENT - metrics[1];
        min_x = min_x.min(left);
        max_x = max_x.max(left + metrics[2]);
        min_y = min_y.min(top);
        max_y = max_y.max(top + metrics[3]);
        x += unicode_advance(&points, index);
    }
    max_x = max_x.max(x.div_euclid(64));
    Some(DynamicLabel {
        points,
        width: max_x - min_x,
        height: max_y - min_y,
        origin_x: min_x,
        origin_y: min_y,
    })
}
pub fn unicode_raster_slice(
    label: &DynamicLabel,
    color: [u8; 3],
    slice_left: i32,
    slice_width: i32,
) -> Vec<u8> {
    let actual_width = slice_width.clamp(0, 1282);
    let actual_height = label.height.clamp(0, 512);
    let length = actual_width as usize * actual_height as usize * 4;
    let mut pixels = Vec::with_capacity(length);
    if length > 0 {
        pixels.extend_from_slice(&[color[0], color[1], color[2], 0]);
        while pixels.len() < length {
            let count = pixels.len().min(length - pixels.len());
            pixels.extend_from_within(0..count);
        }
    }
    let data = dynamic_font_data::unicode_atlas_pixels();
    let mut x = 0i32;
    for index in 0..label.points.len() {
        let metrics = unicode_metrics(label.points[index]);
        let left = x.div_euclid(64) + metrics[0] - label.origin_x - slice_left;
        let top = dynamic_font_data::UNICODE_ASCENT - metrics[1] - label.origin_y;
        if left < actual_width && left + metrics[2] > 0 && metrics[2] > 0 {
            let mut offset = 0;
            let mut at = metrics[5] as usize;
            let end = at + metrics[6] as usize;
            while at < end {
                let run_end = offset + i32::from(data[at]);
                // Transparent runs leave the prefilled hidden RGB unchanged.
                if data[at + 1] == 0 {
                    offset = run_end;
                }
                while offset < run_end {
                    let column = offset % metrics[2];
                    let py = top + offset / metrics[2];
                    let count = (metrics[2] - column).min(run_end - offset);
                    if py >= 0 && py < actual_height {
                        let first = (left + column).max(0);
                        let last = (left + column + count).min(actual_width);
                        for px in first..last {
                            pixels[(py as usize * actual_width as usize + px as usize) * 4 + 3] |=
                                data[at + 1];
                        }
                    }
                    offset += count;
                }
                at += 2;
            }
        }
        x += unicode_advance(&label.points, index);
    }
    pixels
}
