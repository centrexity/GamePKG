#![forbid(unsafe_code)]

//! GamePKG-owned, bounded ZIP compatibility layer for trusted host code.
//!
//! Games never declare this crate. Package archives use the reviewed policies
//! here; arbitrary game-selected paths and unbounded readers stay outside the
//! compatibility surface.

pub use zip_upstream::*;

use std::io::{self, Read, Seek};

pub const MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_ENTRIES: usize = 10_000;

#[derive(Debug)]
pub enum Error {
    Zip(zip_upstream::result::ZipError),
    Io(io::Error),
    Policy(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip(error) => error.fmt(formatter),
            Self::Io(error) => error.fmt(formatter),
            Self::Policy(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Zip(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Policy(_) => None,
        }
    }
}

impl From<zip_upstream::result::ZipError> for Error {
    fn from(error: zip_upstream::result::ZipError) -> Self {
        Self::Zip(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Signed GamePKG packages allow only stored and Zstandard members.
pub fn signed_policy_allows(method: CompressionMethod) -> bool {
    matches!(method, CompressionMethod::Stored | CompressionMethod::Zstd)
}

/// Reads one named entry under an explicit output cap and forces CRC finalization.
pub fn read_entry<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, Error> {
    let cap = max_bytes.min(MAX_ENTRY_BYTES);
    let mut entry = archive.by_name(name)?;
    let declared = entry.size();
    if declared > cap {
        return Err(Error::Policy(format!(
            "ZIP entry {name} declares {declared} bytes above the {cap}-byte cap"
        )));
    }
    let mut output = Vec::with_capacity(declared as usize);
    let mut bounded = entry.by_ref().take(declared.saturating_add(1));
    bounded.read_to_end(&mut output)?;
    if output.len() as u64 != declared {
        return Err(Error::Policy(format!(
            "ZIP entry {name} length differs from its declared size"
        )));
    }
    // Reading one byte past the declared range drives the delegate to EOF,
    // which is where the upstream reader verifies the final CRC.
    let mut probe = [0u8; 1];
    if entry.read(&mut probe)? != 0 {
        return Err(Error::Policy(format!("ZIP entry {name} has trailing data")));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn archive(bytes: &[u8]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = write::SimpleFileOptions::default();
        writer.start_file("payload.txt", options).unwrap();
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn signed_policy_and_bounded_read_are_exact() {
        assert!(signed_policy_allows(CompressionMethod::Stored));
        assert!(signed_policy_allows(CompressionMethod::Zstd));
        assert!(!signed_policy_allows(CompressionMethod::Deflated));

        let packed = archive(b"reviewed payload");
        let mut reader = ZipArchive::new(Cursor::new(packed)).unwrap();
        assert_eq!(
            read_entry(&mut reader, "payload.txt", 64).unwrap(),
            b"reviewed payload"
        );
        assert!(matches!(
            read_entry(&mut reader, "payload.txt", 4),
            Err(Error::Policy(_))
        ));
        assert!(matches!(
            read_entry(&mut reader, "missing.txt", 64),
            Err(Error::Zip(_))
        ));
    }
}
