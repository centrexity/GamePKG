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
    pub fn open(root: &Path, id: &str, creator: &str, schema: u32) -> Result<Self> {
        if schema == 0 {
            bail!("save schema must be positive");
        }
        let scope = if schema == 1 {
            format!("GamePKG save v1\0{id}\0{creator}")
        } else {
            format!("GamePKG save v1\0{id}\0{creator}\0schema={schema}")
        };
        let identity = crate::verify::digest(scope.as_bytes());
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
        let mut save = Save::open(root.path(), "game.one", "creator-one", 1).unwrap();
        save.bytes = b"progress".to_vec();
        save.dirty = true;
        save.commit().unwrap();
        assert_eq!(
            Save::open(root.path(), "game.one", "creator-one", 1)
                .unwrap()
                .bytes,
            b"progress"
        );
        assert!(Save::open(root.path(), "game.two", "creator-one", 1)
            .unwrap()
            .bytes
            .is_empty());
        assert!(Save::open(root.path(), "game.one", "creator-two", 1)
            .unwrap()
            .bytes
            .is_empty());
        let mut new_schema = Save::open(root.path(), "game.one", "creator-one", 2).unwrap();
        assert!(new_schema.bytes.is_empty());
        new_schema.bytes = b"different format".to_vec();
        new_schema.dirty = true;
        new_schema.commit().unwrap();
        assert_eq!(
            Save::open(root.path(), "game.one", "creator-one", 1)
                .unwrap()
                .bytes,
            b"progress"
        );
        assert!(Save::open(root.path(), "game.one", "creator-one", 0).is_err());
        std::fs::write(&save.path, vec![0; MAX_SAVE + 1]).unwrap();
        assert!(Save::open(root.path(), "game.one", "creator-one", 1).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn save_symlinks_do_not_expose_other_files() {
        let root = tempfile::tempdir().unwrap();
        let save = Save::open(root.path(), "one", "key", 1).unwrap();
        let outside = root.path().join("private");
        std::fs::write(&outside, b"private").unwrap();
        std::os::unix::fs::symlink(&outside, &save.path).unwrap();
        assert!(Save::open(root.path(), "one", "key", 1).is_err());
        assert_eq!(std::fs::read(outside).unwrap(), b"private");
    }
}
