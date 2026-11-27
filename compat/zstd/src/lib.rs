#![forbid(unsafe_code)]

//! GamePKG-owned, bounded Zstandard compatibility layer for trusted host code.
//!
//! Games never declare this crate. It exists so signed ZIP assets can use a
//! reviewed in-memory codec boundary with explicit output caps.

pub use zstd_upstream::*;

use std::io::{self, Cursor, Read};

pub const MAX_INPUT_BYTES: usize = 512 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;

/// Compresses bytes with a bounded input size.
pub fn compress(bytes: &[u8], level: i32) -> io::Result<Vec<u8>> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Zstandard input exceeds 512 MiB",
        ));
    }
    zstd_upstream::bulk::compress(bytes, level)
}

/// Decompresses one Zstandard frame with a hard output cap and frame check.
pub fn decompress(bytes: &[u8], max_len: usize) -> io::Result<Vec<u8>> {
    let limit = max_len
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Zstandard limit overflow"))?;
    let mut decoder = stream::read::Decoder::new(Cursor::new(bytes))?;
    let mut output = Vec::new();
    let read = decoder
        .by_ref()
        .take(limit as u64)
        .read_to_end(&mut output)?;
    if read > max_len || output.len() > max_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Zstandard output exceeds bounded limit",
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zstd_round_trip_is_bounded() {
        let source = b"GamePKG bounded Zstandard layer";
        let packed = compress(source, 0).unwrap();
        assert_eq!(decompress(&packed, source.len()).unwrap(), source);
        assert!(decompress(&packed, source.len() - 1).is_err());
    }

    #[test]
    fn zstd_rejects_corrupt_frames_and_oversized_input() {
        let packed = compress(b"frame", 0).unwrap();
        let mut corrupt = packed.clone();
        let index = corrupt.len() / 2;
        corrupt[index] ^= 0xFF;
        assert!(decompress(&corrupt, 64).is_err());
        assert!(decompress(b"not zstd", 64).is_err());
        assert!(compress(&vec![0; MAX_INPUT_BYTES + 1], 0).is_err());
    }
}
