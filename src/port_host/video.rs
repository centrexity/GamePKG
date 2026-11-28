//! Host-owned THP presentation helpers. Games send only decoded plane bytes
//! and logical field numbers; all timing and color conversion stays here.

use super::{timing, Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePlan {
    pub field: u64,
    pub vi_deadline_ns: u64,
    pub dac_callbacks: u32,
}

/// Absolute VI retrace deadline and complete DAC callback count for one field.
/// Arithmetic is checked so a game cannot wrap a deadline into the past.
pub fn frame_plan(standard: u32, start_ns: u64, field: u64) -> Option<FramePlan> {
    let cadence = timing::vi_cadence(standard)?;
    let deadline = timing::vi_deadline(start_ns, field, cadence.period_ns)?;
    Some(FramePlan {
        field,
        vi_deadline_ns: deadline,
        dac_callbacks: timing::dac_timing(standard)?.callback_blocks,
    })
}

/// Convert one linear YCbCr 4:2:0 frame to straight-alpha RGBA8 with the exact
/// integer form of the reviewed four-stage GX/TEV THP presentation path. The
/// GX combine rules scale 255 to 256, add 128 before right-shifting additions
/// and 127 before subtractions, then clamp after the luma stage. Reducing those
/// rules over all byte triples gives the fixed coefficients below.
/// Dimensions are even and bounded; malformed plane sizes are rejected before
/// any output is produced.
pub fn yuv420_to_rgba(width: u32, height: u32, y: &[u8], u: &[u8], v: &[u8]) -> Result<Vec<u8>> {
    if width == 0
        || height == 0
        || width > 2048
        || height > 2048
        || width % 2 != 0
        || height % 2 != 0
    {
        return Err(Error::Invalid);
    }
    let luma_len = (width as usize)
        .checked_mul(height as usize)
        .ok_or(Error::Invalid)?;
    let chroma_len = luma_len / 4;
    if y.len() != luma_len || u.len() != chroma_len || v.len() != chroma_len {
        return Err(Error::Invalid);
    }
    let clamp = |value: i32| -> u8 {
        if value < 0 {
            0
        } else if value > 255 {
            255
        } else {
            value as u8
        }
    };
    let chroma_stride = width as usize / 2;
    let mut out = vec![0u8; luma_len * 4];
    for row in 0..height as usize {
        for column in 0..width as usize {
            let index = row * width as usize + column;
            let chroma = (row / 2) * chroma_stride + column / 2;
            let luma = i32::from(y[index]);
            let blue_chroma = i32::from(u[chroma]);
            let red_chroma = i32::from(v[chroma]);
            let red = luma - 180 + ((360 * red_chroma + 128) >> 8);
            let blue = luma - 228 + 2 * ((227 * blue_chroma + 128) >> 8);
            let green =
                luma + 135 - ((88 * blue_chroma + 127) >> 8) - ((183 * red_chroma + 127) >> 8);
            let pixel = index * 4;
            out[pixel] = clamp(red);
            out[pixel + 1] = clamp(green);
            out[pixel + 2] = clamp(blue);
            out[pixel + 3] = 0xff;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_plan_uses_exact_vi_and_dac_cadence() {
        let plan = frame_plan(timing::VI_NTSC, 100, 2).unwrap();
        assert_eq!(plan.field, 2);
        assert_eq!(plan.vi_deadline_ns, 100 + 2 * 16_683_350);
        assert_eq!(plan.dac_callbacks, 1);
        assert_eq!(frame_plan(99, 0, 1), None);
        assert_eq!(frame_plan(timing::VI_NTSC, u64::MAX, 2), None);
    }
    #[test]
    fn yuv420_conversion_is_bounded_and_deterministic() {
        let y = vec![0u8; 16];
        let u = vec![128u8; 4];
        let v = vec![128u8; 4];
        let rgba = yuv420_to_rgba(4, 4, &y, &u, &v).unwrap();
        assert_eq!(rgba.len(), 16 * 4);
        assert!(rgba.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
        assert!(yuv420_to_rgba(3, 4, &y, &u, &v).is_err());
        assert!(yuv420_to_rgba(4, 4, &y[..15], &u, &v).is_err());
        assert!(yuv420_to_rgba(2050, 4, &y, &u, &v).is_err());
    }
    #[test]
    fn yuv420_uses_exact_gx_tev_rules_and_two_pixel_chroma_blocks() {
        let y = vec![0u8; 16];
        let u = [128u8, 255, 128, 255];
        let v = [128u8, 128, 0, 0];
        let rgba = yuv420_to_rgba(4, 4, &y, &u, &v).unwrap();
        let pixel = |x: usize, y: usize| -> [u8; 4] {
            let at = (y * 4 + x) * 4;
            rgba[at..at + 4].try_into().unwrap()
        };
        assert_eq!(pixel(0, 0), [0, 0, 0, 255]);
        assert_eq!(pixel(1, 1), [0, 0, 0, 255]);
        assert_eq!(pixel(2, 0), [0, 0, 224, 255]);
        assert_eq!(pixel(3, 1), [0, 0, 224, 255]);
        assert_eq!(pixel(0, 2), [0, 91, 0, 255]);
        assert_eq!(pixel(1, 3), [0, 91, 0, 255]);
        assert_eq!(pixel(2, 2), [0, 47, 224, 255]);
        assert_eq!(pixel(3, 3), [0, 47, 224, 255]);
    }
}
