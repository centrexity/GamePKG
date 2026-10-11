//! Reuse host-owned sprite textures without retaining images outside the current frame.
use crate::drawing::image::Image;
use eframe::egui;
use std::sync::Arc;
#[derive(Default)]
pub(super) struct Cache {
    textures: Vec<(Arc<Image>, egui::TextureHandle)>,
}
impl Cache {
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        image: &Arc<Image>,
    ) -> egui::TextureId {
        if let Some((previous, texture)) = self.textures.get_mut(index) {
            if !Arc::ptr_eq(previous, image) && previous.as_ref() != image.as_ref() {
                texture.set(
                    egui::ColorImage::from_rgba_unmultiplied(image.size(), image.pixels()),
                    egui::TextureOptions::NEAREST,
                );
                *previous = image.clone();
            }
        } else {
            // The renderer requests slots in command order, never guest-supplied indices.
            assert_eq!(index, self.textures.len());
            let texture = ctx.load_texture(
                "game-sprite",
                egui::ColorImage::from_rgba_unmultiplied(image.size(), image.pixels()),
                egui::TextureOptions::NEAREST,
            );
            self.textures.push((image.clone(), texture));
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
    fn unchanged_pixels_do_not_upload_again_and_changed_pixels_keep_the_texture_id() {
        let ctx = egui::Context::default();
        let mut cache = Cache::default();
        let red = Arc::new(Image::new(1, 1, &[255, 0, 0, 255]).unwrap());
        let mut id = egui::TextureId::default();
        let first = ctx.run(egui::RawInput::default(), |ctx| {
            id = cache.texture(ctx, 0, &red);
        });
        assert!(first
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        let same = Arc::new(Image::new(1, 1, &[255, 0, 0, 255]).unwrap());
        let second = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(cache.texture(ctx, 0, &same), id);
        });
        assert!(!second
            .textures_delta
            .set
            .iter()
            .any(|(texture, _)| *texture == id));
        let blue = Arc::new(Image::new(1, 1, &[0, 0, 255, 255]).unwrap());
        let third = ctx.run(egui::RawInput::default(), |ctx| {
            assert_eq!(cache.texture(ctx, 0, &blue), id);
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
