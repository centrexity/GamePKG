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
