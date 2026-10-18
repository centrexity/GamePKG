//! Validated RGBA images owned by the host, independent of guest memory.
pub const MAX_DIMENSION: u32 = 512;
pub const MAX_BYTES: usize = 512 * 512 * 4;
pub const MAX_PNG_BYTES: usize = 2 * 1024 * 1024;
/// Session-scoped immutable assets: retain at most one image's maximum pixel
/// budget across up to 32 bounded paths. Frames own separate Arc references and
/// still charge every draw against their normal transfer/command budgets.
#[derive(Default)]
pub(crate) struct Cache {
    entries: std::collections::VecDeque<(String, std::sync::Arc<Image>)>,
    bytes: usize,
}
impl Cache {
    pub(crate) fn get(&mut self, path: &str) -> Option<std::sync::Arc<Image>> {
        let index = self.entries.iter().position(|(name, _)| name == path)?;
        let entry = self.entries.remove(index)?;
        let image = entry.1.clone();
        self.entries.push_back(entry);
        Some(image)
    }
    pub(crate) fn insert(&mut self, path: &str, image: std::sync::Arc<Image>) {
        let bytes = image.pixels().len();
        if path.len() > 1024 || bytes > MAX_BYTES {
            return;
        }
        if let Some(index) = self.entries.iter().position(|(name, _)| name == path) {
            let (_, old) = self.entries.remove(index).unwrap();
            self.bytes -= old.pixels().len();
        }
        while self.entries.len() >= 32 || self.bytes + bytes > MAX_BYTES {
            let (_, old) = self.entries.pop_front().unwrap();
            self.bytes -= old.pixels().len();
        }
        self.bytes += bytes;
        self.entries.push_back((path.to_owned(), image));
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
impl Image {
    pub fn new(width: u32, height: u32, pixels: &[u8]) -> Option<Self> {
        let length = Self::byte_length(width, height)?;
        if pixels.len() != length {
            return None;
        }
        Some(Self {
            width,
            height,
            pixels: pixels.to_vec(),
        })
    }
    pub(crate) fn png_byte_length(header: &[u8]) -> Option<usize> {
        if header.len() < 33
            || &header[..8] != b"\x89PNG\r\n\x1a\n"
            || &header[8..12] != 13u32.to_be_bytes().as_slice()
            || &header[12..16] != b"IHDR"
        {
            return None;
        }
        Self::byte_length(
            u32::from_be_bytes(header[16..20].try_into().ok()?),
            u32::from_be_bytes(header[20..24].try_into().ok()?),
        )
    }
    /// Decode only bounded PNG data, without reading paths or other image formats.
    pub fn from_png(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_PNG_BYTES {
            return None;
        }
        let mut reader = ::image::ImageReader::with_format(
            std::io::Cursor::new(bytes),
            ::image::ImageFormat::Png,
        );
        let mut limits = ::image::Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(8 * 1024 * 1024);
        reader.limits(limits);
        let decoded = reader.decode().ok()?.to_rgba8();
        Self::new(decoded.width(), decoded.height(), decoded.as_raw())
    }
    pub fn byte_length(width: u32, height: u32) -> Option<usize> {
        if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
            return None;
        }
        (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)
    }
    pub fn size(&self) -> [usize; 2] {
        [self.width as usize, self.height as usize]
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_decoding_preserves_pixels_and_rejects_oversized_or_invalid_inputs() {
        use ::image::ImageEncoder;
        let mut bytes = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(
                &[255, 0, 0, 255, 0, 255, 0, 128],
                2,
                1,
                ::image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        let image = Image::from_png(&bytes).unwrap();
        assert_eq!(image.size(), [2, 1]);
        assert_eq!(image.pixels(), &[255, 0, 0, 255, 0, 255, 0, 128]);
        assert!(Image::from_png(b"not an image").is_none());
        assert!(Image::from_png(&bytes[..bytes.len() / 2]).is_none());
        assert!(Image::from_png(&vec![0; MAX_PNG_BYTES + 1]).is_none());
        let mut oversized = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut oversized)
            .write_image(&vec![0; 513 * 4], 513, 1, ::image::ExtendedColorType::Rgba8)
            .unwrap();
        assert!(Image::from_png(&oversized).is_none());
    }
    #[test]
    fn image_is_a_private_exact_size_snapshot() {
        let mut bytes = vec![255, 0, 0, 255, 0, 255, 0, 128];
        let image = Image::new(2, 1, &bytes).unwrap();
        bytes.fill(0);
        assert_eq!(image.size(), [2, 1]);
        assert_eq!(image.pixels(), &[255, 0, 0, 255, 0, 255, 0, 128]);
    }
    #[test]
    fn invalid_dimensions_and_pixel_lengths_are_rejected() {
        for (width, height, bytes) in [
            (0, 1, 0),
            (1, 0, 0),
            (513, 1, 4),
            (1, 513, 4),
            (u32::MAX, u32::MAX, 4),
            (2, 2, 15),
            (2, 2, 17),
        ] {
            assert!(Image::new(width, height, &vec![0; bytes]).is_none());
        }
        assert!(Image::new(512, 512, &vec![0; MAX_BYTES]).is_some());
    }
}

#[cfg(test)]
mod cache_tests {
    use super::{Cache, Image, MAX_BYTES};
    use std::sync::Arc;
    fn image(width: u32, height: u32) -> Arc<Image> {
        Arc::new(
            Image::new(
                width,
                height,
                &vec![37; width as usize * height as usize * 4],
            )
            .unwrap(),
        )
    }
    #[test]
    fn alternating_assets_retain_identity_and_entry_eviction_is_lru() {
        let mut cache = Cache::default();
        let first = image(1, 1);
        cache.insert("0", first.clone());
        for i in 1..32 {
            cache.insert(&i.to_string(), image(1, 1));
        }
        assert!(Arc::ptr_eq(&first, &cache.get("0").unwrap()));
        cache.insert("32", image(1, 1));
        assert_eq!(cache.entries.len(), 32);
        assert!(cache.get("1").is_none());
        assert!(Arc::ptr_eq(&first, &cache.get("0").unwrap()));
    }
    #[test]
    fn pixel_budget_evicts_oldest_without_invalidating_live_frames() {
        let mut cache = Cache::default();
        let retained = image(512, 128);
        for name in ["a", "b", "c", "d"] {
            cache.insert(
                name,
                if name == "b" {
                    retained.clone()
                } else {
                    image(512, 128)
                },
            );
        }
        assert_eq!(cache.bytes, MAX_BYTES);
        cache.get("a").unwrap();
        cache.insert("e", image(1, 1));
        assert!(cache.get("b").is_none());
        assert!(cache.get("a").is_some());
        assert_eq!(cache.bytes, MAX_BYTES - 512 * 128 * 4 + 4);
        assert_eq!(retained.pixels()[0], 37);
        assert_eq!(Arc::strong_count(&retained), 1);
    }
    #[test]
    fn replacement_accounting_maximum_images_and_path_bounds_are_bounded() {
        let mut cache = Cache::default();
        cache.insert("same", image(2, 2));
        let small = image(1, 1);
        cache.insert("same", small.clone());
        assert_eq!((cache.entries.len(), cache.bytes), (1, 4));
        assert!(Arc::ptr_eq(&cache.get("same").unwrap(), &small));
        cache.insert(&"x".repeat(1025), image(1, 1));
        assert_eq!(cache.entries.len(), 1);
        let maximum = image(512, 512);
        cache.insert("large", maximum.clone());
        assert_eq!((cache.entries.len(), cache.bytes), (1, MAX_BYTES));
        assert!(cache.get("same").is_none());
        assert!(Arc::ptr_eq(&maximum, &cache.get("large").unwrap()));
    }
}
