//! Validated RGBA images owned by the host, independent of guest memory.
pub const MAX_DIMENSION: u32 = 512;
pub const MAX_BYTES: usize = 512 * 512 * 4;
pub const MAX_PNG_BYTES: usize = 2 * 1024 * 1024;
/// Session-scoped immutable assets: retain at most one image's maximum pixel
/// budget across up to 32 bounded paths by default; the signed large-image
/// profile permits 128 MiB across 256 paths. Frames own separate Arc references and
/// still charge every draw against their normal transfer/command budgets.
#[derive(Default)]
pub(crate) struct Cache {
    entries: std::collections::VecDeque<(String, std::sync::Arc<Image>)>,
    bytes: usize,
    profile: Option<crate::manifest::RenderProfile>,
}
impl Cache {
    pub(crate) fn for_profile(profile: Option<crate::manifest::RenderProfile>) -> Self {
        Self {
            profile,
            ..Self::default()
        }
    }
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
        let limit = self
            .profile
            .map_or(MAX_BYTES, crate::manifest::RenderProfile::cache_bytes);
        let paths = self
            .profile
            .map_or(32, crate::manifest::RenderProfile::cache_paths);
        while self.entries.len() >= paths || self.bytes + bytes > limit {
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
    fn large_profile_evicts_at_pixel_and_path_bounds_and_preserves_live_frames() {
        let mut cache = Cache::for_profile(Some(crate::manifest::RenderProfile::LargeImagesV1));
        let first = image(512, 512);
        cache.insert("first", first.clone());
        for index in 1..=128 {
            cache.insert(&index.to_string(), image(512, 512));
        }
        assert_eq!(cache.bytes, 128 * 1048576);
        assert!(cache.get("first").is_none());
        assert_eq!(first.pixels()[0], 37);
        assert_eq!(Arc::strong_count(&first), 1);
        let mut tiny = Cache::for_profile(Some(crate::manifest::RenderProfile::LargeImagesV1));
        for index in 0..=256 {
            tiny.insert(&index.to_string(), image(1, 1));
        }
        assert_eq!((tiny.entries.len(), tiny.bytes), (256, 1024));
        assert!(tiny.get("0").is_none());
        assert!(tiny.get("256").is_some());
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

pub(crate) fn asset(
    package: &mut crate::archive::Package,
    cache: &mut Cache,
    frame: &crate::drawing::Frame,
    path: &str,
    bounds: [f32; 4],
) -> Option<std::sync::Arc<Image>> {
    let [x, y, w, h] = bounds;
    if path.len() > 1024 || !crate::archive::valid_path(path) || !frame.can_image(x, y, w, h, 0) {
        return None;
    }
    if let Some(image) = cache.get(path) {
        return frame
            .can_image(x, y, w, h, image.pixels().len())
            .then_some(image);
    }
    // Keep one reader for the complete member: chunk reads restart compressed
    // ZIP entries from the beginning. Validate the small header and frame
    // allowance before allocating, and publish only after the final ZIP CRC.
    let (encoded, pixel_bytes) = package
        .with_asset_reader(path, MAX_PNG_BYTES as u64, |input, length| {
            anyhow::ensure!(length >= 33, "incomplete PNG header");
            let mut header = [0; 33];
            input.read_exact(&mut header)?;
            let Some(pixel_bytes) = Image::png_byte_length(&header) else {
                anyhow::bail!("invalid or oversized PNG header");
            };
            anyhow::ensure!(
                frame.can_image(x, y, w, h, pixel_bytes),
                "image exceeds frame allowance"
            );
            let mut encoded = vec![0; length];
            encoded[..33].copy_from_slice(&header);
            input.read_exact(&mut encoded[33..])?;
            Ok((encoded, pixel_bytes))
        })
        .ok()?;
    let image = Image::from_png(&encoded)?;
    if image.pixels().len() != pixel_bytes {
        return None;
    }
    let image = std::sync::Arc::new(image);
    cache.insert(path, image.clone());
    Some(image)
}

#[cfg(test)]
mod asset_reader_tests {
    use super::*;
    use crate::{archive::Package, drawing::Frame};
    use ::image::ImageEncoder;
    use std::{fs::File, io::Write, sync::Arc};

    fn picture() -> (Vec<u8>, Vec<u8>) {
        let mut value = 0x12345678u32;
        let rgb: Vec<u8> = (0..512 * 512 * 3)
            .map(|_| {
                value ^= value << 13;
                value ^= value >> 17;
                value ^= value << 5;
                value as u8
            })
            .collect();
        let mut png = Vec::new();
        ::image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&rgb, 512, 512, ::image::ExtendedColorType::Rgb8)
            .unwrap();
        assert!(png.len() > 65536 && png.len() < MAX_PNG_BYTES);
        let rgba = rgb
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect();
        (png, rgba)
    }

    fn fixture(
        png: &[u8],
        method: zip::CompressionMethod,
        corrupt_crc: bool,
    ) -> (tempfile::TempDir, Package) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("images.gamepkg");
        let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
        for name in ["picture.png", "next.png"] {
            writer
                .start_file(
                    format!("assets/{name}"),
                    zip::write::SimpleFileOptions::default().compression_method(method),
                )
                .unwrap();
            writer.write_all(png).unwrap();
        }
        writer.finish().unwrap();
        if corrupt_crc {
            // The PNG remains complete and valid. Only its container checksum
            // is wrong, so a decoder alone cannot detect this failure.
            let mut bytes = std::fs::read(&path).unwrap();
            let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
            bytes[central + 16] ^= 1;
            std::fs::write(&path, bytes).unwrap();
        }
        let package = Package::open(&path).unwrap();
        (temp, package)
    }

    #[test]
    fn cold_assets_preserve_all_pixels_across_zip_codecs_and_cache_hits() {
        let (png, rgba) = picture();
        for method in [zip::CompressionMethod::Stored, zip::CompressionMethod::Zstd] {
            let (_temp, mut package) = fixture(&png, method, false);
            package.mark_verified();
            let mut cache = Cache::default();
            let frame = Frame::default();
            let image = asset(
                &mut package,
                &mut cache,
                &frame,
                "picture.png",
                [0., 0., 1., 1.],
            )
            .unwrap();
            assert_eq!(image.pixels(), rgba, "{method:?}");
            let cached = asset(
                &mut package,
                &mut cache,
                &frame,
                "picture.png",
                [0., 0., 1., 1.],
            )
            .unwrap();
            assert!(Arc::ptr_eq(&image, &cached));
            assert_eq!(cache.bytes, MAX_BYTES);
            assert!(frame.commands.is_empty());
        }
    }

    #[test]
    fn complete_png_with_invalid_zip_crc_is_never_published_or_cached() {
        let (png, rgba) = picture();
        for method in [zip::CompressionMethod::Stored, zip::CompressionMethod::Zstd] {
            let (_temp, mut package) = fixture(&png, method, true);
            package.mark_verified();
            let mut cache = Cache::default();
            let frame = Frame::default();
            for _ in 0..2 {
                assert!(
                    asset(
                        &mut package,
                        &mut cache,
                        &frame,
                        "picture.png",
                        [0., 0., 1., 1.]
                    )
                    .is_none(),
                    "{method:?}"
                );
                assert_eq!((cache.bytes, cache.entries.len()), (0, 0));
                assert!(frame.commands.is_empty());
            }
            let valid = asset(
                &mut package,
                &mut cache,
                &frame,
                "next.png",
                [0., 0., 1., 1.],
            )
            .unwrap();
            assert_eq!(
                valid.pixels(),
                rgba,
                "a failed member must not poison the next read"
            );
        }
    }

    #[test]
    fn image_reads_require_verified_paths_and_available_frame_budget() {
        let (png, _) = picture();
        let (_temp, mut package) = fixture(&png, zip::CompressionMethod::Zstd, false);
        let mut cache = Cache::default();
        let mut frame = Frame::default();
        assert!(asset(
            &mut package,
            &mut cache,
            &frame,
            "picture.png",
            [0., 0., 1., 1.]
        )
        .is_none());
        package.mark_verified();
        for path in [
            "../picture.png",
            "assets/picture.png",
            "C:picture.png",
            "picture.png/..",
            "missing.png",
        ] {
            assert!(asset(&mut package, &mut cache, &frame, path, [0., 0., 1., 1.]).is_none());
        }
        let image = Arc::new(Image::new(512, 512, &vec![0; MAX_BYTES]).unwrap());
        for _ in 0..crate::drawing::MAX_IMAGE_BYTES / MAX_BYTES {
            assert!(frame.image_arc(0., 0., 1., 1., image.clone(), false));
        }
        assert!(asset(
            &mut package,
            &mut cache,
            &frame,
            "picture.png",
            [0., 0., 1., 1.]
        )
        .is_none());
        assert_eq!((cache.bytes, cache.entries.len()), (0, 0));
        frame.reset();
        assert!(asset(
            &mut package,
            &mut cache,
            &frame,
            "picture.png",
            [0., 0., 1., 1.]
        )
        .is_some());
    }

    #[test]
    #[ignore = "maintainer cold-image throughput measurement; no timing assertion"]
    fn cold_compressed_image_read_measurement() {
        let (png, rgba) = picture();
        for method in [zip::CompressionMethod::Stored, zip::CompressionMethod::Zstd] {
            let (_temp, mut package) = fixture(&png, method, false);
            package.mark_verified();
            let frame = Frame::default();
            let mut samples = Vec::new();
            for _ in 0..30 {
                let mut cache = Cache::default();
                let start = std::time::Instant::now();
                let image = asset(
                    &mut package,
                    &mut cache,
                    &frame,
                    "picture.png",
                    [0., 0., 1., 1.],
                )
                .unwrap();
                samples.push(start.elapsed().as_micros());
                assert_eq!(image.pixels(), rgba);
            }
            samples.sort_unstable();
            println!(
                "{method:?}: encoded={} samples={} min_us={} median_us={} max_us={}",
                png.len(),
                samples.len(),
                samples[0],
                samples[samples.len() / 2],
                samples[samples.len() - 1]
            );
        }
    }
}
