//! Reuse host-owned sprite textures without retaining images outside the current frame.
use crate::drawing::image::Image;
use eframe::egui;
use std::sync::Arc;
#[derive(Default)]
pub(super) struct Cache {
    textures: Vec<(Arc<Image>, egui::TextureHandle, bool)>,
}
impl Cache {
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        image: &Arc<Image>,
        linear: bool,
    ) -> egui::TextureId {
        let options = if linear {
            egui::TextureOptions::LINEAR
        } else {
            egui::TextureOptions::NEAREST
        };
        if let Some((previous, texture, previous_linear)) = self.textures.get_mut(index) {
            if *previous_linear != linear
                || !Arc::ptr_eq(previous, image) && previous.as_ref() != image.as_ref()
            {
                texture.set(super::game_colors::image(image), options);
                *previous = image.clone();
                *previous_linear = linear;
            }
        } else {
            // The renderer requests slots in command order, never guest-supplied indices.
            assert_eq!(index, self.textures.len());
            let texture =
                ctx.load_texture("game-sprite", super::game_colors::image(image), options);
            self.textures.push((image.clone(), texture, linear));
        }
        self.textures[index].1.id()
    }
    pub fn finish(&mut self, used: usize) {
        self.textures.truncate(used);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_filter_on_identical_pixels_updates_sampling_once() {
        let ctx = egui::Context::default();
        let mut cache = Cache::default();
        let image = Arc::new(Image::new(1, 1, &[255; 4]).unwrap());
        let mut id = egui::TextureId::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            id = cache.texture(ctx, 0, &image, false);
        });
        let changed = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(cache.texture(ctx, 0, &image, true), id);
        });
        assert!(changed.textures_delta.set.iter().any(
            |(texture, delta)| *texture == id && delta.options == egui::TextureOptions::LINEAR
        ));
        let same = ctx.run(egui::RawInput::default(), |ctx| {
            cache.texture(ctx, 0, &image, true);
        });
        assert!(same.textures_delta.set.is_empty());
        let restored = ctx.run(egui::RawInput::default(), |ctx| {
            cache.texture(ctx, 0, &image, false);
        });
        assert!(restored.textures_delta.set.iter().any(
            |(texture, delta)| *texture == id && delta.options == egui::TextureOptions::NEAREST
        ));
    }
    #[test]
    fn unchanged_pixels_do_not_upload_again_and_changed_pixels_keep_the_texture_id() {
        let ctx = egui::Context::default();
        let mut cache = Cache::default();
        let red = Arc::new(Image::new(1, 1, &[255, 0, 0, 255]).unwrap());
        let mut id = egui::TextureId::default();
        let first = ctx.run(egui::RawInput::default(), |ctx| {
            id = cache.texture(ctx, 0, &red, false);
        });
        assert!(first
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        let same = Arc::new(Image::new(1, 1, &[255, 0, 0, 255]).unwrap());
        let second = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(cache.texture(ctx, 0, &same, false), id);
        });
        assert!(!second
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        let blue = Arc::new(Image::new(1, 1, &[0, 0, 255, 255]).unwrap());
        let third = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(cache.texture(ctx, 0, &blue, false), id);
        });
        assert!(third
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        let last = ctx.run(egui::RawInput::default(), |_| {
            cache.finish(0);
        });
        assert!(last.textures_delta.free.contains(&id));
        assert!(cache.textures.is_empty());
    }
}

/// Give one-texel nearest images deterministic physical-pixel coverage.
/// Fractional quads otherwise put sample centers on backend-sensitive edges.
pub(super) fn single_texel_bounds(rect: egui::Rect, pixels_per_point: f32) -> egui::Rect {
    let snap = |value: f32| ((value * pixels_per_point - 0.5).floor() + 1.0) / pixels_per_point;
    egui::Rect::from_min_max(
        egui::pos2(snap(rect.min.x), snap(rect.min.y)),
        egui::pos2(snap(rect.max.x), snap(rect.max.y)),
    )
}

#[cfg(test)]
mod pixel_coverage_tests {
    use super::*;
    #[test]
    fn half_pixel_strips_and_negative_bounds_match_browser_coverage() {
        for (min, max, wanted_min, wanted_max) in [
            ([20.0, 100.0], [620.5, 100.5], [20.0, 100.0], [621.0, 101.0]),
            ([20.5, 100.5], [21.0, 201.0], [21.0, 101.0], [21.0, 201.0]),
            ([-0.25, -0.25], [0.75, 0.75], [0.0, 0.0], [1.0, 1.0]),
            ([-0.5, -0.5], [0.5, 0.5], [0.0, 0.0], [1.0, 1.0]),
            ([0.0, 30.0], [0.5, 31.0], [0.0, 30.0], [1.0, 31.0]),
            ([0.5, 30.0], [1.0, 31.0], [1.0, 30.0], [1.0, 31.0]),
        ] {
            let actual = single_texel_bounds(egui::Rect::from_min_max(min.into(), max.into()), 1.0);
            assert_eq!(actual.min, egui::Pos2::from(wanted_min));
            assert_eq!(actual.max, egui::Pos2::from(wanted_max));
        }
    }
    #[test]
    fn coverage_is_snapped_after_viewport_translation_and_dpi_scale() {
        let actual = single_texel_bounds(
            egui::Rect::from_min_max(egui::pos2(10.25, 20.25), egui::pos2(10.75, 20.75)),
            2.0,
        );
        assert_eq!(actual.min, egui::pos2(10.5, 20.5));
        assert_eq!(actual.max, egui::pos2(11.0, 21.0));
        let fractional = single_texel_bounds(
            egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(11.0, 21.0)),
            1.25,
        );
        assert_eq!(fractional.min, egui::pos2(10.4, 20.0));
        assert_eq!(fractional.max, egui::pos2(11.2, 20.8));
    }
}
