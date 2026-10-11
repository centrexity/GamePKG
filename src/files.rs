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
    let file = options.open(path)?;
    let metadata = file.metadata()?;
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
