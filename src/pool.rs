//! Content-addressed shared assets. Pool contents never grant creator trust.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub sha256: String,
    pub size: u64,
}
impl Reference {
    pub fn validate(&self) -> Result<()> {
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            bail!("asset digest must be 64 lowercase hexadecimal characters");
        }
        if self.size > crate::archive::MAX_FILE {
            bail!("shared asset exceeds 64 MiB");
        }
        Ok(())
    }
}
pub struct Pool {
    directory: PathBuf,
}
impl Pool {
    pub fn open(root: &Path) -> Result<Self> {
        fs::create_dir_all(root)?;
        let directory = root.join("asset-pool");
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                bail!("asset pool must be a regular directory")
            }
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&directory)?
            }
            Err(error) => return Err(error.into()),
        }
        Ok(Self { directory })
    }
    fn path(&self, reference: &Reference) -> Result<PathBuf> {
        reference.validate()?;
        Ok(self.directory.join(format!("{}.blob", reference.sha256)))
    }
    pub fn read(&self, reference: &Reference) -> Result<Vec<u8>> {
        let path = self.path(reference)?;
        let metadata = fs::symlink_metadata(&path).context("shared asset is missing")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("shared asset must be a regular file");
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(path)?;
        if !file.metadata()?.is_file() || file.metadata()?.len() != reference.size {
            bail!("shared asset size mismatch");
        }
        let mut bytes = Vec::new();
        file.take(reference.size + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != reference.size || crate::verify::digest(&bytes) != reference.sha256
        {
            bail!("shared asset content hash mismatch");
        }
        Ok(bytes)
    }
    pub fn put(&self, bytes: &[u8]) -> Result<Reference> {
        let reference = Reference {
            sha256: crate::verify::digest(bytes),
            size: bytes.len() as u64,
        };
        let path = self.path(&reference)?;
        if fs::symlink_metadata(&path).is_ok() {
            self.read(&reference)?;
            return Ok(reference);
        }
        let mut stage = tempfile::NamedTempFile::new_in(&self.directory)?;
        stage.write_all(bytes)?;
        stage.as_file().sync_all()?;
        match stage.persist_noclobber(path) {
            Ok(_) => (),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                self.read(&reference)?;
            }
            Err(error) => return Err(error.error.into()),
        }
        Ok(reference)
    }
    pub fn import_package(
        &self,
        path: &Path,
    ) -> Result<std::collections::BTreeMap<String, Reference>> {
        let mut package = crate::archive::Package::open(path)?;
        crate::verify::verify_embedded(&mut package)?;
        let mut references = std::collections::BTreeMap::new();
        let assets: Vec<_> = package
            .entries
            .keys()
            .filter_map(|name| name.strip_prefix("assets/").map(str::to_owned))
            .collect();
        for asset in assets {
            references.insert(asset.clone(), self.put(&package.asset(&asset)?)?);
        }
        Ok(references)
    }
}
