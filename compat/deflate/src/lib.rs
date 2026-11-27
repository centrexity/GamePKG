#![forbid(unsafe_code)]

//! GamePKG-owned, bounded DEFLATE/zlib compatibility layer for trusted host code.
//!
//! Games never declare this crate. Host audio/WebGL export code uses these
//! bounded functions instead of unbounded direct codec readers.

pub use flate2_upstream::*;

use std::io::{self, Read, Write};

pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

fn bounded_decompress(mut decoder: impl Read, max_len: usize) -> io::Result<Vec<u8>> {
    let limit = max_len.checked_add(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "decompression limit overflow")
    })?;
    let mut output = Vec::new();
    let read = decoder
        .by_ref()
        .take(limit as u64)
        .read_to_end(&mut output)?;
    if read > max_len || output.len() > max_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "decompressed output exceeds bounded limit",
        ));
    }
    Ok(output)
}

/// Compresses bytes with a zlib wrapper.
pub fn zlib_compress(bytes: &[u8], level: Compression) -> io::Result<Vec<u8>> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "zlib input exceeds 64 MiB",
        ));
    }
    let mut encoder = write::ZlibEncoder::new(Vec::new(), level);
    encoder.write_all(bytes)?;
    encoder.finish()
}

/// Decompresses a zlib stream with a hard output cap and full checksum check.
pub fn zlib_decompress(bytes: &[u8], max_len: usize) -> io::Result<Vec<u8>> {
    bounded_decompress(read::ZlibDecoder::new(bytes), max_len)
}

/// Compresses bytes with raw DEFLATE for reviewed native compatibility code.
pub fn raw_deflate_compress(bytes: &[u8], level: Compression) -> io::Result<Vec<u8>> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "DEFLATE input exceeds 64 MiB",
        ));
    }
    let mut encoder = write::DeflateEncoder::new(Vec::new(), level);
    encoder.write_all(bytes)?;
    encoder.finish()
}

/// Decompresses raw DEFLATE with a hard output cap.
pub fn raw_inflate_decompress(bytes: &[u8], max_len: usize) -> io::Result<Vec<u8>> {
    bounded_decompress(read::DeflateDecoder::new(bytes), max_len)
}

/// CRC-32 used by bounded native compatibility shims.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = Crc::new();
    crc.update(bytes);
    crc.sum()
}

/// Conservative bounded upper estimate for zlib output.
pub fn compress_bound(len: usize) -> Option<usize> {
    (len <= MAX_INPUT_BYTES)
        .then(|| len.saturating_mul(2).checked_add(64))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zlib_round_trip_and_crc_are_deterministic() {
        let source = b"GamePKG bounded zlib compatibility layer";
        let packed = zlib_compress(source, Compression::best()).unwrap();
        assert_eq!(zlib_decompress(&packed, source.len()).unwrap(), source);
        assert!(zlib_decompress(&packed, source.len() - 1).is_err());
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn raw_deflate_round_trip_rejects_truncation() {
        let source = vec![7u8; 2048];
        let packed = raw_deflate_compress(&source, Compression::fast()).unwrap();
        assert_eq!(
            raw_inflate_decompress(&packed, source.len()).unwrap(),
            source
        );
        assert!(raw_inflate_decompress(&packed[..packed.len() / 2], source.len()).is_err());
    }

    #[test]
    fn zlib_rejects_corrupt_checksum_and_oversized_input() {
        let packed = zlib_compress(b"checksum", Compression::default()).unwrap();
        let mut corrupt = packed.clone();
        *corrupt.last_mut().unwrap() ^= 0xFF;
        assert!(zlib_decompress(&corrupt, 64).is_err());
        assert!(zlib_compress(&vec![0; MAX_INPUT_BYTES + 1], Compression::none()).is_err());
        assert_eq!(compress_bound(64), Some(192));
    }
}
