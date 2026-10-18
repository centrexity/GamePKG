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
                texture.set(
                    egui::ColorImage::from_rgba_unmultiplied(image.size(), image.pixels()),
                    options,
                );
                *previous = image.clone();
                *previous_linear = linear;
            }
        } else {
            // The renderer requests slots in command order, never guest-supplied indices.
            assert_eq!(index, self.textures.len());
            let texture = ctx.load_texture(
                "game-sprite",
                egui::ColorImage::from_rgba_unmultiplied(image.size(), image.pixels()),
                options,
            );
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
