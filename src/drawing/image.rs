//! Validated RGBA images owned by the host, independent of guest memory.
pub const MAX_DIMENSION: u32 = 512;
pub const MAX_BYTES: usize = 512 * 512 * 4;
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
