#![forbid(unsafe_code)]

//! GamePKG-owned, bounded gzip compatibility layer for trusted host code.
//!
//! Games never declare this crate. SDK, updater, demo and native-service host
//! code uses these bounded functions or the reviewed streaming adapters below.

pub use flate2_upstream::*;

use std::io::{self, Read, Write};

pub const MAX_INPUT_BYTES: usize = 512 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024 * 1024;

fn bounded_decode(mut decoder: impl Read, max_len: usize) -> io::Result<Vec<u8>> {
    let limit = max_len
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "gzip limit overflow"))?;
    let mut output = Vec::new();
    let read = decoder
        .by_ref()
        .take(limit as u64)
        .read_to_end(&mut output)?;
    if read > max_len || output.len() > max_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "gzip output exceeds bounded limit",
        ));
    }
    Ok(output)
}

/// Encodes bytes as a single bounded gzip member.
pub fn encode(bytes: &[u8], level: Compression) -> io::Result<Vec<u8>> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gzip input exceeds 512 MiB",
        ));
    }
    let mut encoder = write::GzEncoder::new(Vec::new(), level);
    encoder.write_all(bytes)?;
    encoder.finish()
}

/// Decodes one gzip member, checking CRC/ISIZE and a hard output cap.
pub fn decode(bytes: &[u8], max_len: usize) -> io::Result<Vec<u8>> {
    bounded_decode(read::GzDecoder::new(bytes), max_len)
}

/// Decodes concatenated gzip members with a combined hard output cap.
pub fn decode_multi(bytes: &[u8], max_len: usize) -> io::Result<Vec<u8>> {
    bounded_decode(read::MultiGzDecoder::new(bytes), max_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_single_member_is_bounded_and_verified() {
        let source = b"GamePKG bounded gzip layer";
        let packed = encode(source, Compression::fast()).unwrap();
        assert_eq!(decode(&packed, source.len()).unwrap(), source);
        assert!(decode(&packed, source.len() - 1).is_err());
        assert!(decode(&packed[..packed.len() - 2], source.len()).is_err());
    }

    #[test]
    fn gzip_multi_member_decodes_with_one_total_limit() {
        let first = encode(b"first", Compression::fast()).unwrap();
        let second = encode(b"second", Compression::fast()).unwrap();
        let mut joined = first;
        joined.extend_from_slice(&second);
        assert_eq!(decode_multi(&joined, 11).unwrap(), b"firstsecond");
        assert!(decode_multi(&joined, 10).is_err());
        assert_eq!(decode(&joined, 11).unwrap(), b"first");
    }
}
