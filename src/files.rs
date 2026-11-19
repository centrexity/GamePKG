//! Bounded snapshots of regular host files, including explicitly selected ROMs.
use anyhow::{bail, Context, Result};
use std::{fs, io::Read, path::Path};

pub(crate) fn open_regular(path: &Path, limit: u64, follow_link: bool) -> Result<fs::File> {
    let metadata = if follow_link {
        fs::metadata(path)
    } else {
        fs::symlink_metadata(path)
    }?;
    if !metadata.is_file() || metadata.len() > limit {
        bail!("file must be regular and at most {limit} bytes");
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | if follow_link { 0 } else { libc::O_NOFOLLOW });
    }
    #[cfg(windows)]
    if !follow_link {
        use std::os::windows::fs::OpenOptionsExt;
        // Open the reparse point itself, including if it replaced the checked path.
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    #[cfg(windows)]
    if !follow_link {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            bail!("file must not be a Windows reparse point");
        }
    }

    if !metadata.is_file() || metadata.len() > limit {
        bail!("file must be regular and at most {limit} bytes");
    }
    Ok(file)
}

pub(crate) fn read_regular(path: &Path, limit: u64, follow_link: bool) -> Result<Vec<u8>> {
    let file = open_regular(path, limit, follow_link)?;
    let mut bytes = Vec::new();
    file.take(limit.checked_add(1).context("invalid file size limit")?)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("file exceeds {limit} bytes");
    }
    Ok(bytes)
}

pub(crate) fn hash_archive(path: &Path) -> Result<String> {
    crate::verify::digest_reader(open_regular(path, u64::MAX, false)?)
}

pub(crate) fn snapshot_archive(path: &Path) -> Result<tempfile::NamedTempFile> {
    let mut snapshot = tempfile::NamedTempFile::new()?;
    std::io::copy(&mut open_regular(path, u64::MAX, false)?, &mut snapshot)?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_regular_snapshots_reject_directories_and_large_files() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        std::fs::write(&path, b"test").unwrap();
        assert_eq!(super::read_regular(&path, 4, false).unwrap(), b"test");
        assert!(super::read_regular(&path, 3, false).is_err());
        assert!(super::read_regular(root.path(), 4, false).is_err());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn cache_links_are_rejected_but_explicit_selected_links_are_allowed() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("file");
        let link = root.path().join("link");
        std::fs::write(&path, b"selected ROM").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&path, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&path, &link).unwrap();
        assert!(super::read_regular(&link, 64, false).is_err());
        assert_eq!(
            super::read_regular(&link, 64, true).unwrap(),
            b"selected ROM"
        );
    }
}

/// A selected disc stays open read-only; reads are bounded independently of its size.
#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
pub(crate) const MAX_NATIVE_ROM_BYTES: u64 = 1 << 40;
const MAX_ROM_READ: usize = 65536;
const LEGACY_ROM_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) enum RomSet {
    Files(Vec<(std::sync::Mutex<fs::File>, u64)>),
    Snapshots(Vec<Vec<u8>>),
}

impl Default for RomSet {
    fn default() -> Self {
        Self::Files(Vec::new())
    }
}

impl RomSet {
    pub fn open(paths: &[std::path::PathBuf], cap: u64, snapshot: bool) -> Result<Self> {
        if snapshot {
            let mut total = 0usize;
            let mut roms = Vec::with_capacity(paths.len());
            for (index, path) in paths.iter().enumerate() {
                let bytes = read_regular(path, LEGACY_ROM_BYTES, true).with_context(|| {
                    format!("unable to open ROM {} for the legacy engine", index + 1)
                })?;
                if bytes.is_empty() {
                    bail!("ROM {} is empty", index + 1);
                }
                total += bytes.len();
                if total > 2 * LEGACY_ROM_BYTES as usize {
                    bail!("combined legacy ROM snapshots exceed 128 MiB");
                }
                roms.push(bytes);
            }
            return Ok(Self::Snapshots(roms));
        }
        paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                let file = open_regular(path, cap, true).with_context(|| {
                    format!("unable to open ROM {} (limit {cap} bytes)", index + 1)
                })?;
                let len = file.metadata()?.len();
                if len == 0 || len > cap {
                    bail!("ROM {} must contain 1 to {cap} bytes", index + 1);
                }
                Ok((std::sync::Mutex::new(file), len))
            })
            .collect::<Result<Vec<_>>>()
            .map(Self::Files)
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Files(files) => files.len(),
            Self::Snapshots(roms) => roms.len(),
        }
    }

    pub fn size(&self, index: usize) -> Option<u64> {
        match self {
            Self::Files(files) => files.get(index).map(|(_, size)| *size),
            Self::Snapshots(roms) => roms.get(index).map(|data| data.len() as u64),
        }
    }

    pub fn snapshots(&self) -> Option<&[Vec<u8>]> {
        match self {
            Self::Snapshots(roms) => Some(roms),
            Self::Files(_) => None,
        }
    }

    pub fn read(&self, index: usize, offset: u64, len: usize) -> Option<Vec<u8>> {
        let available = self.size(index)?.checked_sub(offset)?;
        if len > MAX_ROM_READ {
            return None;
        }
        let count = available.min(len as u64) as usize;
        match self {
            Self::Snapshots(roms) => {
                let start = usize::try_from(offset).ok()?;
                Some(roms.get(index)?.get(start..start + count)?.to_vec())
            }
            Self::Files(files) => {
                let (file, _) = files.get(index)?;
                let mut file = file.lock().ok()?;
                use std::io::{Seek, SeekFrom};
                file.seek(SeekFrom::Start(offset)).ok()?;
                let mut bytes = vec![0; count];
                file.read_exact(&mut bytes).ok()?;
                Some(bytes)
            }
        }
    }
}
