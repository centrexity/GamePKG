#![forbid(unsafe_code)]

//! GamePKG-owned, bounded PNG compatibility layer for trusted host code.
//!
//! This crate is only a host dependency. Games never declare it, cannot import
//! it, and receive only host-owned image handles through `gamepkg::` APIs. The
//! upstream image codec remains hidden behind the bounded functions below.

pub use image_upstream::*;

pub const MAX_DIMENSION: u32 = 512;
pub const MAX_BYTES: usize = 512 * 512 * 4;
pub const MAX_PNG_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_DECODE_ALLOC_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub enum Error {
    InputTooLarge { len: usize, max: usize },
    InvalidDimensions { width: u32, height: u32 },
    PixelLengthMismatch { len: usize, expected: usize },
    Upstream(image_upstream::ImageError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InputTooLarge { len, max } => {
                write!(formatter, "PNG input of {len} bytes exceeds {max} bytes")
            }
            Self::InvalidDimensions { width, height } => {
                write!(formatter, "invalid PNG dimensions {width}x{height}")
            }
            Self::PixelLengthMismatch { len, expected } => {
                write!(formatter, "RGBA length {len} does not match {expected}")
            }
            Self::Upstream(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Upstream(error) => Some(error),
            _ => None,
        }
    }
}

impl From<image_upstream::ImageError> for Error {
    fn from(error: image_upstream::ImageError) -> Self {
        Self::Upstream(error)
    }
}

fn checked_dimensions(width: u32, height: u32) -> Result<usize, Error> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(Error::InvalidDimensions { width, height });
    }
    let length = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(Error::InvalidDimensions { width, height })?;
    if length > MAX_BYTES {
        return Err(Error::InvalidDimensions { width, height });
    }
    Ok(length)
}

/// Decodes in-memory PNG bytes into an exact RGBA8 buffer.
pub fn decode_png(bytes: &[u8]) -> Result<RgbaImage, Error> {
    if bytes.len() > MAX_PNG_BYTES {
        return Err(Error::InputTooLarge {
            len: bytes.len(),
            max: MAX_PNG_BYTES,
        });
    }
    let mut reader = ImageReader::with_format(std::io::Cursor::new(bytes), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC_BYTES);
    reader.limits(limits);
    let decoded = reader.decode()?.into_rgba8();
    let (width, height) = decoded.dimensions();
    let expected = checked_dimensions(width, height)?;
    let raw = decoded.into_raw();
    if raw.len() != expected {
        return Err(Error::PixelLengthMismatch {
            len: raw.len(),
            expected,
        });
    }
    RgbaImage::from_raw(width, height, raw).ok_or(Error::PixelLengthMismatch { len: 0, expected })
}

/// Encodes an exact RGBA8 buffer as bounded PNG bytes.
pub fn encode_png_rgba(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, Error> {
    let expected = checked_dimensions(width, height)?;
    if pixels.len() != expected {
        return Err(Error::PixelLengthMismatch {
            len: pixels.len(),
            expected,
        });
    }
    let mut bytes = Vec::new();
    codecs::png::PngEncoder::new(&mut bytes).write_image(
        pixels,
        width,
        height,
        ExtendedColorType::Rgba8,
    )?;
    Ok(bytes)
}

/// Resizes host-owned RGBA data with an explicit upstream filter.
pub fn resize_rgba(
    image: &RgbaImage,
    width: u32,
    height: u32,
    filter: imageops::FilterType,
) -> Result<RgbaImage, Error> {
    checked_dimensions(width, height)?;
    Ok(imageops::resize(image, width, height, filter))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
        let pixels = vec![color; (width as usize) * (height as usize)];
        let mut raw = Vec::with_capacity(pixels.len() * 4);
        for pixel in pixels {
            raw.extend_from_slice(&pixel);
        }
        encode_png_rgba(width, height, &raw).unwrap()
    }

    #[test]
    fn bounded_png_round_trip_preserves_exact_rgba() {
        let source = png(3, 2, [10, 20, 30, 40]);
        let decoded = decode_png(&source).unwrap();
        assert_eq!(decoded.dimensions(), (3, 2));
        assert_eq!(decoded.as_raw().len(), 3 * 2 * 4);
        assert_eq!(
            encode_png_rgba(3, 2, decoded.as_raw()).unwrap().len(),
            source.len()
        );
    }

    #[test]
    fn bounded_png_rejects_oversized_input_dimensions_and_pixels() {
        assert!(matches!(
            decode_png(&vec![0; MAX_PNG_BYTES + 1]),
            Err(Error::InputTooLarge { .. })
        ));
        assert!(matches!(
            encode_png_rgba(MAX_DIMENSION + 1, 1, &[]),
            Err(Error::InvalidDimensions { .. })
        ));
        assert!(matches!(
            encode_png_rgba(2, 2, &[0; 15]),
            Err(Error::PixelLengthMismatch { .. })
        ));
        assert!(decode_png(b"not a PNG").is_err());
    }
}
