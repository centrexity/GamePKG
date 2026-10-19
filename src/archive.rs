use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use zip::ZipArchive;
#[path = "archive_file.rs"]
mod archive_file;
use archive_file::CursorFile;
use std::sync::Arc;

#[derive(Clone)]
struct SharedSnapshot {
    // Close the last cursor before deleting the private snapshot on Windows.
    cursor: CursorFile,
    _owner: Arc<tempfile::NamedTempFile>,
}
impl SharedSnapshot {
    fn new(file: tempfile::NamedTempFile) -> Result<Self> {
        Ok(Self {
            cursor: CursorFile::new(file.as_file().try_clone()?),
            _owner: Arc::new(file),
        })
    }
}

pub const MAX_FILE: u64 = 64 * 1024 * 1024;
pub const MAX_TOTAL: u64 = 256 * 1024 * 1024;
// Virtual names also become compiler staging paths. Apply portable component
// rules before any host path is constructed; do not normalize attacker input.
pub fn valid_path(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 1024
        && name.split('/').count() <= 32
        && !name.contains('\\')
        && !name.contains(':')
        && !name.starts_with('/')
        && name.split('/').all(|p| {
            let base = p
                .split('.')
                .next()
                .unwrap()
                .trim_end_matches(' ')
                .to_ascii_uppercase();
            let device = matches!(
                base.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            ) || ["COM", "LPT"].iter().any(|prefix| {
                base.strip_prefix(prefix).is_some_and(|digit| {
                    matches!(
                        digit,
                        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                    )
                })
            });
            !p.is_empty()
                && p.len() <= 255
                && !p.ends_with(['.', ' '])
                && !device
                && !p.chars().any(|c| c.is_control() || "<>\"|?*".contains(c))
        })
}

fn register_path(paths: &mut BTreeMap<String, (String, bool)>, name: &str) -> Result<()> {
    let mut prefix = String::new();
    let mut parts = name.split('/').peekable();
    while let Some(part) = parts.next() {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        let file = parts.peek().is_none();
        let folded = prefix.to_lowercase();
        if let Some((previous, was_file)) = paths.get(&folded) {
            if previous != &prefix || *was_file || file {
                bail!("ambiguous archive path: {name}");
            }
        } else {
            paths.insert(folded, (prefix.clone(), file));
        }
    }
    Ok(())
}
pub struct Package {
    archive: ZipArchive<CursorFile>,
    pub entries: BTreeMap<String, u64>,
    raw: CursorFile,
    verified: bool,
    shared_refs: BTreeMap<String, crate::pool::Reference>,
    shared_files: BTreeMap<String, SharedSnapshot>,
}
impl Package {
    pub fn open(path: &Path) -> Result<Self> {
        let raw_file = CursorFile::new(File::open(path)?);
        let mut archive = ZipArchive::new(raw_file.clone())?;
        // zip's name index collapses duplicates. Count raw central records before
        // using that index, so duplicate names cannot disappear during parsing.
        let mut raw = raw_file.clone();
        raw.seek(SeekFrom::Start(archive.central_directory_start()))?;
        let mut count = 0usize;
        loop {
            let mut magic = [0u8; 4];
            raw.read_exact(&mut magic)?;
            if magic != *b"PK\x01\x02" {
                break;
            }
            count += 1;
            if count > 10000 {
                bail!("too many central directory entries");
            }
            let mut header = [0u8; 42];
            raw.read_exact(&mut header)?;
            let skip = [24, 26, 28]
                .iter()
                .map(|&i| u16::from_le_bytes([header[i], header[i + 1]]) as i64)
                .sum::<i64>();
            raw.seek(SeekFrom::Current(skip))?;
        }
        if count != archive.len() {
            bail!("duplicate or inconsistent archive entries");
        }
        if archive.len() > 10000 {
            bail!("too many archive entries");
        }
        let mut entries = BTreeMap::new();
        let mut portable_paths = BTreeMap::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            let f = archive.by_index(i)?;
            if !valid_path(f.name()) || f.is_dir() || f.is_symlink() {
                bail!("invalid archive path: {}", f.name());
            }
            if (f.name().starts_with("source/") || f.name().starts_with("shaders/"))
                && !f.name().is_ascii()
            {
                bail!(
                    "compiler source paths must be ASCII to avoid filesystem normalization aliases"
                );
            }
            register_path(&mut portable_paths, f.name())?;
            if !matches!(
                f.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Zstd
            ) {
                bail!("unsupported compression");
            }
            if f.size() > MAX_FILE {
                bail!("entry too large");
            }
            total = total
                .checked_add(f.size())
                .context("archive size overflow")?;
            if total > MAX_TOTAL {
                bail!("archive too large");
            }
            if entries.insert(f.name().to_owned(), f.size()).is_some() {
                bail!("duplicate archive entry");
            }
        }
        Ok(Self {
            archive,
            entries,
            raw: raw_file,
            verified: false,
            shared_refs: BTreeMap::new(),
            shared_files: BTreeMap::new(),
        })
    }
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        if !valid_path(path) {
            bail!("invalid virtual path");
        }
        let f = self
            .archive
            .by_name(path)
            .with_context(|| format!("unable to read package file {path}"))?;
        let expected = f.size();
        let mut data = Vec::new();
        f.take(expected + 1).read_to_end(&mut data)?;
        if data.len() as u64 != expected {
            bail!("entry size does not match archive metadata");
        }
        Ok(data)
    }
    /// Share verified descriptors and snapshots with a background reader.
    /// No archive path is reopened, and each reader has independent positions.
    pub fn try_clone_verified(&self) -> Result<Self> {
        if !self.verified {
            bail!("package reader clone requires verification");
        }
        Ok(Self {
            archive: self.archive.clone(),
            entries: self.entries.clone(),
            raw: self.raw.clone(),
            verified: true,
            shared_refs: self.shared_refs.clone(),
            shared_files: self.shared_files.clone(),
        })
    }
    pub fn has_shared_assets(&self) -> bool {
        !self.shared_refs.is_empty()
    }
    pub(crate) fn set_shared_refs(&mut self, refs: BTreeMap<String, crate::pool::Reference>) {
        self.shared_refs = refs;
        self.shared_files.clear();
    }
    /// Resolve signed references into private session snapshots, never host paths.
    pub fn resolve_shared_assets(&mut self, root: &Path) -> Result<()> {
        if !self.verified {
            bail!("shared assets require a verified package");
        }
        if self.shared_refs.is_empty() {
            return Ok(());
        }
        let pool = crate::pool::Pool::open(root)?;
        let mut files = BTreeMap::new();
        for reference in self.shared_refs.values() {
            if files.contains_key(&reference.sha256) {
                continue;
            }
            let bytes = pool.read(reference)?;
            let mut snapshot = tempfile::NamedTempFile::new()?;
            std::io::Write::write_all(&mut snapshot, &bytes)?;
            files.insert(reference.sha256.clone(), SharedSnapshot::new(snapshot)?);
        }
        self.shared_files = files;
        Ok(())
    }
    pub(crate) fn mark_verified(&mut self) {
        self.verified = true;
    }
    pub(crate) fn is_verified(&self) -> bool {
        self.verified
    }
    /// Read a bounded range from an already verified package without extracting it.
    pub fn asset_chunk(&mut self, path: &str, offset: u64, length: usize) -> Result<Vec<u8>> {
        if !self.verified || !valid_path(path) || length > 65536 {
            bail!("asset range requires a verified package, valid path, and at most 64 KiB");
        }
        if let Some(reference) = self.shared_refs.get(path) {
            if offset > reference.size {
                bail!("shared asset offset exceeds size");
            }
            let file = &mut self
                .shared_files
                .get_mut(&reference.sha256)
                .context("shared asset is not resolved")?
                .cursor;
            file.seek(SeekFrom::Start(offset))?;
            let mut bytes = vec![0; length.min((reference.size - offset) as usize)];
            file.read_exact(&mut bytes)?;
            return Ok(bytes);
        }
        let mut entry = self.archive.by_name(&format!("assets/{path}"))?;
        if offset > entry.size() {
            bail!("asset offset exceeds entry size");
        }
        let length = length.min((entry.size() - offset) as usize);
        let mut bytes = vec![0; length];
        if entry.compression() == zip::CompressionMethod::Stored {
            let start = entry
                .data_start()
                .checked_add(offset)
                .context("asset offset overflow")?;
            drop(entry);
            self.raw.seek(SeekFrom::Start(start))?;
            self.raw.read_exact(&mut bytes)?;
        } else {
            let skipped = std::io::copy(&mut entry.by_ref().take(offset), &mut std::io::sink())?;
            if skipped != offset {
                bail!("asset ended before requested offset");
            }
            entry.read_exact(&mut bytes)?;
        }
        Ok(bytes)
    }
    pub(crate) fn asset_size(&self, path: &str) -> Result<u64> {
        if !self.verified || !valid_path(path) {
            bail!("asset size requires a verified package and valid path");
        }
        if let Some(reference) = self.shared_refs.get(path) {
            if !self.shared_files.contains_key(&reference.sha256) {
                bail!("shared asset is unavailable");
            }
            return Ok(reference.size);
        }
        self.entries
            .get(&format!("assets/{path}"))
            .copied()
            .context("asset is missing")
    }
    pub(crate) fn asset_compression(
        &mut self,
        path: &str,
    ) -> Result<Option<zip::CompressionMethod>> {
        self.asset_size(path)?;
        if self.shared_refs.contains_key(path) {
            return Ok(None);
        }
        let index = self
            .archive
            .index_for_name(&format!("assets/{path}"))
            .context("asset is missing")?;
        // Inspect metadata without constructing a decompressor. Callers use
        // this result before reserving any codec workspace.
        Ok(Some(self.archive.by_index_raw(index)?.compression()))
    }
    /// Borrow a bounded reader over a verified asset without extracting it or
    /// allocating a complete host-side copy. The borrow ends before returning.
    pub fn with_asset_reader<T>(
        &mut self,
        path: &str,
        maximum: u64,
        read: impl FnOnce(&mut dyn Read, usize) -> Result<T>,
    ) -> Result<T> {
        let size = self.asset_size(path)?;
        if maximum > MAX_FILE || size > maximum {
            bail!("asset exceeds reader limit");
        }
        let length = usize::try_from(size).context("asset length exceeds address space")?;
        if let Some(reference) = self.shared_refs.get(path) {
            let file = &mut self
                .shared_files
                .get_mut(&reference.sha256)
                .context("shared asset is not resolved")?
                .cursor;
            if file.length()? != size {
                bail!("shared asset size changed");
            }
            file.seek(SeekFrom::Start(0))?;
            return read(&mut file.take(size), length);
        }
        let mut entry = self.archive.by_name(&format!("assets/{path}"))?;
        if entry.size() != size {
            bail!("asset size changed");
        }
        let mut bounded = entry.by_ref().take(size);
        let value = read(&mut bounded, length)?;
        let complete = bounded.limit() == 0;
        // Take's EOF must not suppress the ZIP reader's final CRC check.
        // Partial consumers may return early; complete consumers must see the
        // underlying EOF before a result (such as a decoder) is published.
        if complete && entry.read(&mut [0u8; 1])? != 0 {
            bail!("asset exceeds declared size");
        }
        Ok(value)
    }
    pub fn asset(&mut self, path: &str) -> Result<Vec<u8>> {
        if !valid_path(path) {
            bail!("invalid asset path");
        }
        if let Some(reference) = self.shared_refs.get(path) {
            let size = reference.size;
            self.asset_chunk(path, 0, 0)?;
            let mut bytes = Vec::new();
            for offset in (0..size).step_by(65536) {
                bytes.extend(self.asset_chunk(path, offset, 65536)?);
            }
            return Ok(bytes);
        }
        self.read(&format!("assets/{path}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn verified_clones_read_concurrently_without_reopening_or_sharing_positions() {
        for method in [zip::CompressionMethod::Stored, zip::CompressionMethod::Zstd] {
            let temporary = tempfile::tempdir().unwrap();
            let path = temporary.path().join("parallel.gamepkg");
            let a = Arc::new((0..262147).map(|i| (i % 251) as u8).collect::<Vec<_>>());
            let b = Arc::new(
                (0..131123)
                    .map(|i| (255 - i % 239) as u8)
                    .collect::<Vec<_>>(),
            );
            let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
            for (name, bytes) in [("a", &a), ("b", &b)] {
                writer
                    .start_file(
                        format!("assets/{name}"),
                        zip::write::SimpleFileOptions::default().compression_method(method),
                    )
                    .unwrap();
                writer.write_all(bytes).unwrap();
            }
            writer.finish().unwrap();
            let mut package = Package::open(&path).unwrap();
            assert!(package.try_clone_verified().is_err());
            assert!(package.asset_compression("a").is_err());
            package.mark_verified();
            assert_eq!(package.asset_compression("a").unwrap(), Some(method));
            assert!(package.asset_compression("missing").is_err());
            assert!(package.asset_compression("../a").is_err());
            let first = package.try_clone_verified().unwrap();
            let second = package.try_clone_verified().unwrap();
            let barrier = Arc::new(std::sync::Barrier::new(3));
            let workers = [first, second]
                .into_iter()
                .enumerate()
                .map(|(worker, mut reader)| {
                    let a = a.clone();
                    let b = b.clone();
                    let barrier = barrier.clone();
                    std::thread::spawn(move || {
                        barrier.wait();
                        for n in 0..128 {
                            let (name, expected) = if (n + worker) % 2 == 0 {
                                ("a", &a)
                            } else {
                                ("b", &b)
                            };
                            let start = n * 7919 % expected.len();
                            let length = 1024.min(expected.len() - start);
                            assert_eq!(
                                reader.asset_chunk(name, start as u64, length).unwrap(),
                                expected[start..start + length]
                            );
                            if n % 16 == 0 {
                                reader
                                    .with_asset_reader(name, MAX_FILE, |input, size| {
                                        let mut bytes = vec![0; size];
                                        input.read_exact(&mut bytes)?;
                                        assert_eq!(bytes, **expected);
                                        Ok(())
                                    })
                                    .unwrap();
                            }
                        }
                    })
                })
                .collect::<Vec<_>>();
            barrier.wait();
            for _ in 0..32 {
                assert_eq!(package.asset_chunk("a", 1, 4096).unwrap(), a[1..4097]);
            }
            for worker in workers {
                worker.join().unwrap();
            }
            // Cache-path replacement must not redirect a verified clone's descriptor.
            let mut clone = package.try_clone_verified().unwrap();
            let replacement = temporary.path().join("replacement");
            std::fs::write(&replacement, b"not the verified archive").unwrap();
            std::fs::rename(&replacement, &path).unwrap();
            drop(package);
            assert_eq!(clone.asset_chunk("b", 0, 16).unwrap(), b[..16]);
        }
    }
    #[test]
    fn cloned_shared_snapshots_live_until_the_last_reader_drops() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("shared.gamepkg");
        zip::ZipWriter::new(File::create(&path).unwrap())
            .finish()
            .unwrap();
        let mut package = Package::open(&path).unwrap();
        package.mark_verified();
        let digest = "12".repeat(32);
        package.set_shared_refs(BTreeMap::from([(
            "data".to_owned(),
            crate::pool::Reference {
                sha256: digest.clone(),
                size: 10,
            },
        )]));
        let mut snapshot = tempfile::NamedTempFile::new().unwrap();
        snapshot.write_all(b"abcdefghij").unwrap();
        let snapshot_path = snapshot.path().to_owned();
        package
            .shared_files
            .insert(digest, SharedSnapshot::new(snapshot).unwrap());
        let mut clone = package.try_clone_verified().unwrap();
        drop(package);
        assert!(snapshot_path.exists());
        assert_eq!(clone.asset_chunk("data", 3, 4).unwrap(), b"defg");
        clone
            .with_asset_reader("data", 10, |input, _| {
                let mut bytes = [0; 10];
                input.read_exact(&mut bytes)?;
                assert_eq!(&bytes, b"abcdefghij");
                Ok(())
            })
            .unwrap();
        drop(clone);
        assert!(!snapshot_path.exists());
    }
    #[test]
    fn complete_asset_reader_checks_crc_without_a_consumer_eof_probe() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("crc.gamepkg");
        let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
        writer
            .start_file(
                "assets/data",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(b"valid").unwrap();
        writer.finish().unwrap();
        let mut package = Package::open(&path).unwrap();
        package.mark_verified();
        let start = package.archive.by_name("assets/data").unwrap().data_start();
        let mut changed = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        changed.seek(SeekFrom::Start(start)).unwrap();
        changed.write_all(b"X").unwrap();
        let budget = crate::memory_budget::Budget::new(1);
        let mut resource_created = false;
        assert!(package
            .with_asset_reader("data", 5, |input, size| {
                assert_eq!(size, 5);
                let mut bytes = [0; 5];
                input.read_exact(&mut bytes)?;
                // Do not request EOF. A post-consumer CRC failure must also
                // destroy the returned resource and release its reservation.
                resource_created = true;
                budget.reserve(1)
            })
            .is_err());
        assert!(resource_created);
        assert_eq!(budget.used(), 0);
        changed.seek(SeekFrom::Start(start)).unwrap();
        changed.write_all(b"v").unwrap();
        assert_eq!(
            package
                .with_asset_reader("data", 5, |input, _| {
                    let mut bytes = [0; 5];
                    input.read_exact(&mut bytes)?;
                    Ok(bytes)
                })
                .unwrap(),
            *b"valid"
        );
    }
    #[test]
    fn borrowed_asset_readers_require_verification_limits_and_scoped_paths() {
        for method in [zip::CompressionMethod::Stored, zip::CompressionMethod::Zstd] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("reader.gamepkg");
            let bytes = (0..200123).map(|i| (i % 251) as u8).collect::<Vec<_>>();
            let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
            writer
                .start_file(
                    "assets/data",
                    zip::write::SimpleFileOptions::default().compression_method(method),
                )
                .unwrap();
            writer.write_all(&bytes).unwrap();
            writer.finish().unwrap();
            let mut package = Package::open(&path).unwrap();
            let mut called = false;
            assert!(package
                .with_asset_reader("data", MAX_FILE, |_, _| {
                    called = true;
                    Ok(())
                })
                .is_err());
            assert!(!called);
            package.mark_verified();
            for bad in [
                "../data",
                "/data",
                "data\\..",
                "C:data",
                "data/../data",
                "source/Cargo.toml",
                "assets/data",
                "missing",
            ] {
                assert!(
                    package
                        .with_asset_reader(bad, MAX_FILE, |_, _| {
                            called = true;
                            Ok(())
                        })
                        .is_err(),
                    "{bad}"
                );
                assert!(!called);
            }
            for limit in [0, bytes.len() as u64 - 1, MAX_FILE + 1] {
                assert!(package
                    .with_asset_reader("data", limit, |_, _| {
                        called = true;
                        Ok(())
                    })
                    .is_err());
                assert!(!called);
            }
            assert!(package
                .with_asset_reader::<()>("data", MAX_FILE, |input, _| {
                    input.read_exact(&mut [0; 7])?;
                    bail!("injected reader cancellation")
                })
                .is_err());
            for _ in 0..2 {
                package
                    .with_asset_reader("data", bytes.len() as u64, |input, length| {
                        assert_eq!(length, bytes.len());
                        let mut chunk = [0; 65536];
                        let mut at = 0;
                        loop {
                            let count = input.read(&mut chunk)?;
                            if count == 0 {
                                break;
                            }
                            assert_eq!(&chunk[..count], &bytes[at..at + count]);
                            at += count;
                        }
                        assert_eq!(at, bytes.len());
                        Ok(())
                    })
                    .unwrap();
            }
            assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 1);
            let digest = "12".repeat(32);
            package.set_shared_refs(BTreeMap::from([(
                "shared".to_owned(),
                crate::pool::Reference {
                    sha256: digest.clone(),
                    size: 3,
                },
            )]));
            assert!(package
                .with_asset_reader("shared", MAX_FILE, |_, _| {
                    called = true;
                    Ok(())
                })
                .is_err());
            assert!(!called);
            let mut snapshot = tempfile::NamedTempFile::new().unwrap();
            snapshot.write_all(b"abc").unwrap();
            package
                .shared_files
                .insert(digest, SharedSnapshot::new(snapshot).unwrap());
            for _ in 0..2 {
                package
                    .with_asset_reader("shared", 3, |input, length| {
                        assert_eq!(length, 3);
                        let mut output = [0; 3];
                        input.read_exact(&mut output)?;
                        assert_eq!(&output, b"abc");
                        assert_eq!(input.read(&mut [0])?, 0);
                        Ok(())
                    })
                    .unwrap();
            }
            let mut changed = package
                .shared_files
                .values_mut()
                .next()
                .unwrap()
                ._owner
                .as_file()
                .try_clone()
                .unwrap();
            changed.seek(SeekFrom::End(0)).unwrap();
            changed.write_all(b" beyond signed size").unwrap();
            assert!(package
                .with_asset_reader("shared", MAX_FILE, |_, _| {
                    called = true;
                    Ok(())
                })
                .is_err());
            assert!(!called);
        }
    }
    #[test]
    fn asset_ranges_are_bounded_and_require_verification() {
        for method in [zip::CompressionMethod::Stored, zip::CompressionMethod::Zstd] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("ranges.gamepkg");
            let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
            writer
                .start_file(
                    "assets/data",
                    zip::write::SimpleFileOptions::default().compression_method(method),
                )
                .unwrap();
            writer.write_all(b"abcdefghij").unwrap();
            writer.finish().unwrap();
            let mut package = Package::open(&path).unwrap();
            assert!(package.asset_chunk("data", 2, 3).is_err());
            assert_eq!(package.asset("data").unwrap(), b"abcdefghij");
            package.mark_verified();
            assert_eq!(package.asset_chunk("data", 2, 3).unwrap(), b"cde");
            assert_eq!(package.asset_chunk("data", 8, 50).unwrap(), b"ij");
            assert!(package.asset_chunk("data", 10, 3).unwrap().is_empty());
            assert!(package.asset_chunk("data", 11, 3).is_err());
            assert!(package.asset_chunk("../data", 0, 3).is_err());
            assert!(package.asset_chunk("data", 0, 65537).is_err());
            assert_eq!(package.asset("data").unwrap(), b"abcdefghij");
        }
    }
}
