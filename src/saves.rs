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
    max_bytes: usize,
    pending: Option<(usize, Vec<u8>)>,
}
impl Save {
    pub fn open_for_game(root: &Path, manifest: &crate::manifest::Manifest) -> Result<Self> {
        if manifest.builtin_engine.as_deref() == Some(crate::doom::PROFILE) {
            Self::open_with_limit(
                root,
                &manifest.id,
                &manifest.creator_key,
                manifest.save_schema,
                crate::doom::SAVE_MAX,
            )
        } else {
            Self::open(
                root,
                &manifest.id,
                &manifest.creator_key,
                manifest.save_schema,
            )
        }
    }
    pub fn open(root: &Path, id: &str, creator: &str, schema: u32) -> Result<Self> {
        Self::open_with_limit(root, id, creator, schema, MAX_SAVE)
    }
    pub(crate) fn open_with_limit(
        root: &Path,
        id: &str,
        creator: &str,
        schema: u32,
        max_bytes: usize,
    ) -> Result<Self> {
        if max_bytes == 0 || max_bytes > crate::doom::SAVE_MAX {
            bail!("invalid scoped save limit");
        }
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
                file.take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if bytes.len() > max_bytes {
            bail!("save exceeds its scoped limit");
        }
        Ok(Self {
            bytes,
            dirty: false,
            path,
            max_bytes,
            pending: None,
        })
    }
    pub fn discard_pending(&mut self) {
        self.pending = None;
    }
    /// Ordered chunks form one replacement blob, published only when complete.
    pub fn chunk(&mut self, offset: usize, total: usize, bytes: &[u8]) -> bool {
        if total > MAX_SAVE
            || bytes.len() > 65536
            || offset
                .checked_add(bytes.len())
                .is_none_or(|end| end > total)
            || (bytes.is_empty() && total != 0)
        {
            self.pending = None;
            return false;
        }
        if offset == 0 {
            self.pending = Some((total, Vec::with_capacity(total)));
        }
        let Some((expected, pending)) = self.pending.as_mut() else {
            return false;
        };
        if *expected != total || pending.len() != offset {
            self.pending = None;
            return false;
        }
        pending.extend_from_slice(bytes);
        if pending.len() == total {
            let (_, bytes) = self.pending.take().unwrap();
            self.bytes = bytes;
            self.dirty = true;
        }
        true
    }
    pub fn commit(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if self.bytes.len() > self.max_bytes {
            bail!("save exceeds its scoped limit");
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
    fn chunked_replacements_are_bounded_ordered_and_atomic() {
        let root = tempfile::tempdir().unwrap();
        let mut save = Save::open(root.path(), "chunked", "creator", 1).unwrap();
        save.bytes = b"old".to_vec();
        assert!(save.chunk(0, 6, b"abc"));
        assert_eq!(save.bytes, b"old");
        assert!(!save.dirty);
        assert!(!save.chunk(4, 6, b"ef"));
        assert!(!save.chunk(3, 6, b"def"));
        assert!(save.chunk(0, 6, b"abc"));
        save.discard_pending();
        assert!(!save.chunk(3, 6, b"def"));
        assert!(save.chunk(0, 6, b"abc"));
        assert!(save.chunk(3, 6, b"def"));
        assert_eq!(save.bytes, b"abcdef");
        save.commit().unwrap();
        for (offset, total, bytes) in [
            (0, MAX_SAVE + 1, &b"x"[..]),
            (usize::MAX, MAX_SAVE, &b"x"[..]),
            (0, 1, &b""[..]),
        ] {
            assert!(!save.chunk(offset, total, bytes));
            assert_eq!(save.bytes, b"abcdef");
        }
        let full = vec![7; 65536];
        for offset in (0..MAX_SAVE).step_by(65536) {
            assert!(save.chunk(offset, MAX_SAVE, &full));
            if offset + 65536 < MAX_SAVE {
                assert_eq!(save.bytes, b"abcdef");
            }
        }
        assert_eq!(save.bytes, vec![7; MAX_SAVE]);
        assert!(!save.chunk(0, MAX_SAVE, &vec![0; 65537]));
        assert!(save.chunk(0, 0, b""));
        assert!(save.bytes.is_empty());
        assert_eq!(
            Save::open(root.path(), "chunked", "creator", 1)
                .unwrap()
                .bytes,
            b"abcdef"
        );
    }
    #[test]
    fn larger_doom_storage_does_not_expand_ordinary_save_limits() {
        let root = tempfile::tempdir().unwrap();
        let mut manifest: crate::manifest::Manifest =
            toml::from_str(include_str!("../ports/doom/manifest.toml")).unwrap();
        let mut save = Save::open_for_game(root.path(), &manifest).unwrap();
        save.bytes = vec![0x5a; MAX_SAVE + 1];
        save.dirty = true;
        save.commit().unwrap();
        assert_eq!(
            Save::open_for_game(root.path(), &manifest)
                .unwrap()
                .bytes
                .len(),
            MAX_SAVE + 1
        );
        manifest.builtin_engine = None;
        assert!(Save::open_for_game(root.path(), &manifest).is_err());
        let ordinary = tempfile::tempdir().unwrap();
        let mut save = Save::open(ordinary.path(), "ordinary", "creator", 1).unwrap();
        save.bytes = b"previous save".to_vec();
        save.dirty = true;
        save.commit().unwrap();
        save.bytes = vec![0; MAX_SAVE + 1];
        save.dirty = true;
        assert!(save.commit().is_err());
        assert_eq!(
            Save::open(ordinary.path(), "ordinary", "creator", 1)
                .unwrap()
                .bytes,
            b"previous save"
        );
    }
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
