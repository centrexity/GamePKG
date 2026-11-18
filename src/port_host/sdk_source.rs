//! Private snapshot of the source bundle pinned by the app build.
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
};

const MAX_COMPRESSED: u64 = 512 * 1024 * 1024;
const MAX_PAYLOAD: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 100_000;

pub(super) struct Source {
    root: PathBuf,
    pin: Option<&'static str>,
    _private: Option<tempfile::TempDir>,
    _lock: Option<File>,
}

impl Source {
    pub(super) fn open(cache: &Path) -> Result<Self> {
        match option_env!("GAMEPKG_NATIVE_SDK_PIN") {
            None => Ok(Self {
                root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
                pin: None,
                _private: None,
                _lock: None,
            }),
            Some(pin) => {
                let app = std::env::current_exe()?;
                let prefix = app
                    .parent()
                    .and_then(Path::parent)
                    .context("native SDK requires the GamePKG installation directory")?;
                let archive = prefix.join("share/gamepkg/native-service-sdk.tar.gz");
                let installed = snapshot(&archive, pin).with_context(|| {
                    format!("verify installed native SDK {}", archive.display())
                })?;
                let base = cache.join("native-runtime-builds");
                private_dir(&base)?;
                let store = base.join(pin);
                private_dir(&store)?;
                let store = store.canonicalize()?;
                let lock = lock(&store)?;
                let root = store.join("source");
                materialize(installed, &store, &root)?;
                Ok(Self {
                    root,
                    pin: Some(pin),
                    _private: Some(tempfile::tempdir()?),
                    _lock: Some(lock),
                })
            }
        }
    }

    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    pub(super) fn pin(&self) -> Option<&'static str> {
        self.pin
    }

    pub(super) fn private_cargo_home(&self) -> Option<PathBuf> {
        self._private
            .as_ref()
            .map(|temp| temp.path().join("cargo-home"))
    }
}

fn private_dir(path: &Path) -> Result<()> {
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o077 == 0,
        "native SDK cache must be a private directory"
    );
    Ok(())
}

fn lock(store: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(store.join("source.lock"))?;
    ensure!(file.metadata()?.is_file(), "invalid native SDK cache lock");
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX) } == 0 {
            return Ok(file);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error.into());
        }
    }
}

fn snapshot(path: &Path, pin: &str) -> Result<File> {
    // Copy then verify. Extraction reads precisely the bytes that were hashed.
    let mut original =
        crate::files::open_regular(path, MAX_COMPRESSED, false)?.take(MAX_COMPRESSED + 1);
    let mut snapshot = tempfile::tempfile()?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut block = [0u8; 64 * 1024];
    loop {
        let count = original.read(&mut block)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        ensure!(
            total <= MAX_COMPRESSED,
            "native SDK compressed size limit exceeded"
        );
        hash.update(&block[..count]);
        snapshot.write_all(&block[..count])?;
    }
    ensure!(
        hex::encode(hash.finalize()) == pin,
        "native SDK hash differs from app build"
    );
    snapshot.rewind()?;
    Ok(snapshot)
}

fn materialize(archive: impl Read, store: &Path, root: &Path) -> Result<()> {
    match fs::symlink_metadata(root) {
        Ok(metadata) => {
            ensure!(metadata.is_dir(), "invalid cached native SDK directory");
            let mut files = BTreeSet::new();
            inventory(root, &mut files)?;
            walk_verified(archive, |relative, bytes, size| {
                let path = root.join(relative);
                ensure!(
                    files.remove(&path),
                    "native SDK cache is missing {}",
                    relative.display()
                );
                let mut file = crate::files::open_regular(&path, size, false)?;
                let metadata = file.metadata()?;
                ensure!(
                    metadata.len() == size && metadata.nlink() == 1,
                    "cached native SDK file changed: {}",
                    relative.display()
                );
                compare(bytes, &mut file, size).with_context(|| {
                    format!("verify cached native SDK file {}", relative.display())
                })
            })?;
            ensure!(
                files.is_empty(),
                "native SDK cache contains unexpected files"
            );
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let stage = tempfile::tempdir_in(store)?;
            let staged = stage.path().join("source");
            fs::create_dir(&staged)?;
            walk_verified(archive, |relative, bytes, size| {
                let destination = staged.join(relative);
                fs::create_dir_all(destination.parent().context("invalid native SDK parent")?)?;
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(destination)?;
                ensure!(
                    io::copy(bytes, &mut file)? == size,
                    "truncated native SDK file"
                );
                Ok(())
            })?;
            fs::rename(staged, root)?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn inventory(root: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    let mut dirs = vec![root.to_path_buf()];
    let mut entries = 0;
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            entries += 1;
            ensure!(
                entries <= MAX_ENTRIES,
                "native SDK cache exceeds entry limit"
            );
            let kind = entry.file_type()?;
            if kind.is_dir() {
                dirs.push(entry.path());
            } else {
                ensure!(
                    kind.is_file(),
                    "native SDK cache contains a link or special file"
                );
                files.insert(entry.path());
            }
        }
    }
    Ok(())
}

fn compare(left: &mut dyn Read, right: &mut dyn Read, mut remaining: u64) -> Result<()> {
    let mut expected = [0; 16 * 1024];
    let mut actual = [0; 16 * 1024];
    while remaining != 0 {
        let size = remaining.min(expected.len() as u64) as usize;
        left.read_exact(&mut expected[..size])?;
        right.read_exact(&mut actual[..size])?;
        ensure!(
            expected[..size] == actual[..size],
            "cached source differs from pinned archive"
        );
        remaining -= size as u64;
    }
    Ok(())
}

fn walk_verified(
    archive: impl Read,
    mut member: impl FnMut(&Path, &mut dyn Read, u64) -> Result<()>,
) -> Result<()> {
    let compressed = flate2::read::GzDecoder::new(archive);
    // Include a bounded allowance for tar headers and long names.
    let mut archive = tar::Archive::new(compressed.take(MAX_PAYLOAD + 256 * 1024 * 1024));
    let mut total = 0u64;
    for (index, entry) in archive.entries()?.enumerate() {
        let mut entry = entry?;
        ensure!(index < MAX_ENTRIES, "too many native SDK files");
        ensure!(
            entry.header().entry_type().is_file(),
            "native SDK links and special files are forbidden"
        );
        let path = entry.path()?.into_owned();
        let name = path.to_str().context("invalid native SDK file name")?;
        ensure!(
            name.len() <= 2048
                && !name.contains('\\')
                && name.split('/').all(|part| !part.is_empty()
                    && part != "."
                    && part != ".."
                    && !part.chars().any(char::is_control)),
            "invalid native SDK member path"
        );
        let size = entry.size();
        total = total
            .checked_add(size)
            .context("native SDK size overflow")?;
        ensure!(
            total <= MAX_PAYLOAD,
            "native SDK unpacked size limit exceeded"
        );
        member(&path, &mut entry, size)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        let content = b"reviewed";
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(content.len() as u64);
        header.set_cksum();
        archive
            .append_data(&mut header, ".cargo/config.toml", &content[..])
            .unwrap();
        archive.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn source_cache_reuses_exact_files_and_rejects_changed_or_extra_inputs() {
        let store = tempfile::tempdir().unwrap();
        let root = store.path().join("source");
        let config = root.join(".cargo/config.toml");
        let archive = fixture();
        let load = || materialize(&archive[..], store.path(), &root);
        load().unwrap();
        let modified = config.metadata().unwrap().modified().unwrap();
        load().unwrap();
        assert_eq!(config.metadata().unwrap().modified().unwrap(), modified);
        fs::write(&config, b"tampered").unwrap();
        assert!(load().is_err());
        fs::write(&config, b"reviewed").unwrap();
        fs::write(root.join(".cargo/helper"), b"extra").unwrap();
        assert!(load().is_err());
        fs::remove_file(root.join(".cargo/helper")).unwrap();
        std::os::unix::fs::symlink("config.toml", root.join(".cargo/link")).unwrap();
        assert!(load().is_err());
    }
}
