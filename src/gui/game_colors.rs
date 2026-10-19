//! Game RGBA uses conventional source-over in encoded RGB, matching Canvas/SDL.
//! egui's UI helper premultiplies in linear light, so it is unsuitable here.
use eframe::egui;

pub(super) fn rgba(r: u8, g: u8, b: u8, a: u8) -> egui::Color32 {
    let premultiply = |value: u8| ((u16::from(value) * u16::from(a) + 127) / 255) as u8;
    egui::Color32::from_rgba_premultiplied(premultiply(r), premultiply(g), premultiply(b), a)
}
pub(super) fn packed(value: u32) -> egui::Color32 {
    rgba(
        (value >> 24) as u8,
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
    )
}
pub(super) fn image(image: &crate::drawing::image::Image) -> egui::ColorImage {
    egui::ColorImage {
        size: image.size(),
        pixels: image
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| rgba(p[0], p[1], p[2], p[3]))
            .collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::{image, packed, rgba};
    #[test]
    fn game_color_vectors_keep_encoded_alpha_instead_of_ui_light_conversion() {
        for (source, expected) in [
            (0xffffff1e, [30, 30, 30, 30]),
            (0x00c8ffc8, [0, 157, 200, 200]),
            (0x2896dc50, [13, 47, 69, 80]),
            (0x8246c800, [0, 0, 0, 0]),
            (0x0e5ca7ff, [14, 92, 167, 255]),
        ] {
            assert_eq!(packed(source).to_array(), expected);
        }
    }
    #[test]
    fn all_channel_alpha_pairs_are_bounded_and_opaque_pixels_are_unchanged() {
        for a in 0..=255 {
            for value in 0..=255 {
                let color = rgba(value, 255 - value, value / 2, a).to_array();
                assert!(color[..3].iter().all(|c| *c <= a));
                assert_eq!(color[3], a);
                if a == 255 {
                    assert_eq!(color, [value, 255 - value, value / 2, 255]);
                }
            }
        }
    }
    #[test]
    fn image_conversion_uses_identical_game_color_rules_without_mutating_source() {
        let pixels = [255, 255, 255, 30, 14, 92, 167, 255, 130, 70, 200, 0];
        let source = crate::drawing::image::Image::new(3, 1, &pixels).unwrap();
        let converted = image(&source);
        assert_eq!(converted.size, [3, 1]);
        assert_eq!(
            converted
                .pixels
                .iter()
                .map(|p| p.to_array())
                .collect::<Vec<_>>(),
            vec![[30, 30, 30, 30], [14, 92, 167, 255], [0, 0, 0, 0]]
        );
        assert_eq!(source.pixels(), pixels);
    }
}
