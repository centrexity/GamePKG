//! One bounded blob per signed creator and game identity; guests never choose a path.
use anyhow::{bail, Result};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const MAX_SAVE: usize = 1024 * 1024;
pub(crate) struct Save {
    pub bytes: Vec<u8>,
    pub dirty: bool,
    path: PathBuf,
}
impl Save {
    pub fn open(root: &Path, id: &str, creator: &str) -> Result<Self> {
        let identity =
            crate::verify::digest(format!("GamePKG save v1\0{id}\0{creator}").as_bytes());
        let directory = root.join("saves");
        if directory
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink())
        {
            bail!("save folder cannot be a symlink");
        }
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{identity}.blob"));
        if path
            .symlink_metadata()
            .is_ok_and(|m| m.file_type().is_symlink() || !m.is_file())
        {
            bail!("save must be a regular file");
        }
        let mut bytes = Vec::new();
        match std::fs::File::open(&path) {
            Ok(file) => {
                file.take(MAX_SAVE as u64 + 1).read_to_end(&mut bytes)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if bytes.len() > MAX_SAVE {
            bail!("save exceeds 1 MiB");
        }
        Ok(Self {
            bytes,
            dirty: false,
            path,
        })
    }
    pub fn commit(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        let mut temp = tempfile::NamedTempFile::new_in(self.path.parent().unwrap())?;
        temp.write_all(&self.bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path).map_err(|e| e.error)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_identity_is_scoped_and_oversized_files_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let mut save = Save::open(root.path(), "game.one", "creator-one").unwrap();
        save.bytes = b"progress".to_vec();
        save.dirty = true;
        save.commit().unwrap();
        assert_eq!(
            Save::open(root.path(), "game.one", "creator-one")
                .unwrap()
                .bytes,
            b"progress"
        );
        assert!(Save::open(root.path(), "game.two", "creator-one")
            .unwrap()
            .bytes
            .is_empty());
        assert!(Save::open(root.path(), "game.one", "creator-two")
            .unwrap()
            .bytes
            .is_empty());
        std::fs::write(&save.path, vec![0; MAX_SAVE + 1]).unwrap();
        assert!(Save::open(root.path(), "game.one", "creator-one").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn save_symlinks_do_not_expose_other_files() {
        let root = tempfile::tempdir().unwrap();
        let save = Save::open(root.path(), "one", "key").unwrap();
        let outside = root.path().join("private");
        std::fs::write(&outside, b"private").unwrap();
        std::os::unix::fs::symlink(&outside, &save.path).unwrap();
        assert!(Save::open(root.path(), "one", "key").is_err());
        assert_eq!(std::fs::read(outside).unwrap(), b"private");
    }
}
