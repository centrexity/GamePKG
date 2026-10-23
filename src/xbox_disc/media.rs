//! Owned, budgeted streams over a verified disc. Every returned data block is
//! checked again, including when the image is changed without changing its size.
use super::{read_at, Disc, Entry, Report, MANIFEST};
use crate::memory_budget::{Budget, Lease};
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::Arc,
};

pub const BLOCK: usize = 65536;
// Indexing admits at most8192 paths of <512 bytes, a1MiB table and bounded
// worklists. This transient reservation covers that maximum before parsing.
const OPEN_MEMORY: u64 = 32 * 1024 * 1024;
// The pinned716-file manifest needs <1.3MiB of block hashes; this also covers
// file/directory metadata, allocation headers, the shared owner and its report.
const INDEX_MEMORY: u64 = 4 * 1024 * 1024;
const STREAM_MEMORY: u64 = (2 * BLOCK + 4096) as u64;

struct VerifiedEntry {
    entry: Entry,
    hashes: Vec<[u8; 32]>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub is_directory: bool,
    pub size: u64,
}
struct Media {
    file: File,
    length: u64,
    files: Vec<VerifiedEntry>,
    lookup: BTreeMap<String, usize>,
    directories: BTreeMap<String, Vec<DirectoryEntry>>,
    report: Report,
    budget: Budget,
    _index: Lease,
}
#[derive(Clone)]
pub struct VerifiedDisc(Arc<Media>);
pub struct Stream {
    media: Arc<Media>,
    index: usize,
    position: u64,
    scratch: Vec<u8>,
    cached: [Option<usize>; 2],
    _memory: Lease,
}
fn relative(path: &str, root_allowed: bool) -> Result<String> {
    ensure!(
        path.len() < 512 && !path.chars().any(|c| c.is_control() || c == ':'),
        "invalid Xbox resource path"
    );
    if path.is_empty() {
        ensure!(root_allowed, "file path is empty");
        return Ok(String::new());
    }
    let path = path.replace('\\', "/");
    ensure!(
        path.split('/').all(|part| !matches!(part, "" | "." | "..")),
        "Xbox resource path must stay relative to the disc"
    );
    Ok(path.to_ascii_lowercase())
}
impl VerifiedDisc {
    /// The supplied budget must belong to the game that will own this media.
    /// No independent allowance is created and no player asset is extracted.
    pub fn open(path: &Path, budget: &Budget) -> Result<Self> {
        let opening = budget
            .reserve(OPEN_MEMORY)
            .context("Xbox disc index memory reservation")?;
        let disc = Disc::open(path)?;
        let media = Self::prepare(disc, MANIFEST, budget)?;
        ensure!(
            media.report().verified_files == 716 && media.report().verified_bytes == 2476107072,
            "pinned disc manifest is incomplete"
        );
        drop(opening);
        Ok(media)
    }
    fn prepare(disc: Disc, manifest: &str, budget: &Budget) -> Result<Self> {
        let index_memory = budget.reserve(INDEX_MEMORY)?;
        let mut files = Vec::new();
        let mut lookup = BTreeMap::new();
        let mut hash_bytes = 0u64;
        for line in manifest
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let mut fields = line.splitn(3, ' ');
            let _hash = fields.next().context("invalid pinned disc manifest")?;
            let size: u64 = fields
                .next()
                .context("invalid pinned disc manifest")?
                .parse()?;
            let path = fields.next().context("invalid pinned disc manifest")?;
            ensure!(files.len() < 716, "too many pinned disc resources");
            let entry = disc
                .file(path)
                .with_context(|| format!("missing disc resource {path}"))?
                .clone();
            ensure!(
                u64::from(entry.size) == size,
                "unsupported or damaged disc: wrong file size for {path}"
            );
            hash_bytes += size.div_ceil(BLOCK as u64) * 32;
            ensure!(
                hash_bytes + (files.len() as u64 + 1) * 3072 + 65536 <= INDEX_MEMORY,
                "verified disc index exceeds its memory reservation"
            );
            let key = relative(path, false)?;
            ensure!(
                lookup.insert(key, files.len()).is_none(),
                "duplicate pinned resource"
            );
            let mut hashes = Vec::new();
            hashes.try_reserve_exact((entry.size as usize).div_ceil(BLOCK))?;
            files.push(VerifiedEntry { entry, hashes });
        }
        let report = disc.verify_blocks(manifest, |path, bytes| {
            let index = *lookup
                .get(&path.to_ascii_lowercase())
                .context("resource absent from pinned index")?;
            files[index].hashes.push(Sha256::digest(bytes).into());
            Ok(())
        })?;
        ensure!(
            files
                .iter()
                .all(|file| file.hashes.len() == (file.entry.size as usize).div_ceil(BLOCK)),
            "verified disc block index is incomplete"
        );
        let mut directories = BTreeMap::<String, Vec<DirectoryEntry>>::new();
        directories.insert(String::new(), Vec::new());
        for file in &files {
            let parts = file.entry.path.split('/').collect::<Vec<_>>();
            let mut parent = String::new();
            for (index, name) in parts.iter().enumerate() {
                let is_directory = index + 1 < parts.len();
                let children = directories.entry(parent.clone()).or_default();
                if let Some(existing) = children
                    .iter()
                    .find(|child| child.name.eq_ignore_ascii_case(name))
                {
                    ensure!(
                        existing.is_directory == is_directory,
                        "disc file/directory conflict"
                    );
                } else {
                    children.push(DirectoryEntry {
                        name: (*name).to_owned(),
                        is_directory,
                        size: if is_directory {
                            0
                        } else {
                            u64::from(file.entry.size)
                        },
                    });
                }
                if !parent.is_empty() {
                    parent.push('/');
                }
                parent.push_str(&name.to_ascii_lowercase());
                if is_directory {
                    directories.entry(parent.clone()).or_default();
                }
            }
        }
        for children in directories.values_mut() {
            children.sort_by_cached_key(|child| child.name.to_ascii_lowercase());
        }
        Ok(Self(Arc::new(Media {
            file: disc.file,
            length: disc.length,
            files,
            lookup,
            directories,
            report,
            budget: budget.clone(),
            _index: index_memory,
        })))
    }
    pub fn report(&self) -> &Report {
        &self.0.report
    }
    /// Borrowed metadata only; unrelated files in the image are not exposed.
    pub fn directory(&self, path: &str) -> Result<&[DirectoryEntry]> {
        let key = relative(path, true)?;
        Ok(self
            .0
            .directories
            .get(&key)
            .context("disc directory is unavailable")?)
    }
    pub fn open_stream(&self, path: &str) -> Result<Stream> {
        let key = relative(path, false)?;
        let index = *self
            .0
            .lookup
            .get(&key)
            .context("file is not in the verified disc manifest")?;
        let memory = self.0.budget.reserve(STREAM_MEMORY)?;
        let mut scratch = Vec::new();
        scratch.try_reserve_exact(2 * BLOCK)?;
        scratch.resize(2 * BLOCK, 0);
        Ok(Stream {
            media: self.0.clone(),
            index,
            position: 0,
            scratch,
            cached: [None; 2],
            _memory: memory,
        })
    }
}
impl Stream {
    pub fn len(&self) -> u64 {
        u64::from(self.media.files[self.index].entry.size)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn position(&self) -> u64 {
        self.position
    }
    fn checked_read(&mut self, output: &mut [u8]) -> Result<usize> {
        let file = &self.media.files[self.index];
        let size = u64::from(file.entry.size);
        let count = (size - self.position).min(output.len().min(BLOCK) as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        let first = self.position as usize / BLOCK;
        let last = (self.position as usize + count - 1) / BLOCK;
        // At most two blocks. Both are checked before the caller's buffer or
        // stream position changes, so even a failed cross-block read is atomic.
        // Cache hits return the checked bytes themselves, never unchecked new
        // data. Do not evict either requested page while filling a cache miss.
        let mut slots = [0usize; 2];
        for block in first..=last {
            if let Some(slot) = self.cached.iter().position(|cached| *cached == Some(block)) {
                slots[block - first] = slot;
                continue;
            }
            let slot = self
                .cached
                .iter()
                .position(|cached| cached.is_none_or(|page| page < first || page > last))
                .context("verified disc cache invariant")?;
            self.cached[slot] = None;
            let start = block * BLOCK;
            let length = (size as usize - start).min(BLOCK);
            let bytes = &mut self.scratch[slot * BLOCK..slot * BLOCK + length];
            read_at(
                &self.media.file,
                self.media.length,
                file.entry.offset + start as u64,
                bytes,
            )?;
            ensure!(
                <[u8; 32]>::from(Sha256::digest(&*bytes)) == file.hashes[block],
                "Xbox disc resource changed after verification: {}",
                file.entry.path
            );
            self.cached[slot] = Some(block);
            slots[block - first] = slot;
        }
        let start = self.position as usize % BLOCK;
        let first_count = count.min(BLOCK - start);
        let offset = slots[0] * BLOCK + start;
        output[..first_count].copy_from_slice(&self.scratch[offset..offset + first_count]);
        if first_count != count {
            let offset = slots[1] * BLOCK;
            output[first_count..count]
                .copy_from_slice(&self.scratch[offset..offset + count - first_count]);
        }
        self.position += count as u64;
        Ok(count)
    }
}
impl Read for Stream {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.checked_read(output).map_err(io::Error::other)
    }
}
impl Seek for Stream {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.len().checked_add_signed(delta),
        }
        .filter(|position| *position <= self.len())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek is outside the verified disc resource",
            )
        })?;
        self.position = position;
        Ok(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    const BASE: u64 = (1u64 << 31) + 17;
    fn fixture() -> (tempfile::NamedTempFile, Disc, String, Vec<u8>) {
        let mut image = tempfile::NamedTempFile::new().unwrap();
        let data = (0..2 * BLOCK + 13)
            .map(|n| (n.wrapping_mul(37) ^ (n >> 9)) as u8)
            .collect::<Vec<_>>();
        let length = BASE + data.len() as u64 + 5;
        image.as_file().set_len(length).unwrap();
        image.seek(SeekFrom::Start(BASE)).unwrap();
        image.write_all(&data).unwrap();
        image.write_all(b"small").unwrap();
        image.flush().unwrap();
        let entries = [
            Entry {
                path: "Data/pattern.bin".into(),
                offset: BASE,
                size: data.len() as u32,
            },
            Entry {
                path: "Data/small.bin".into(),
                offset: BASE + data.len() as u64,
                size: 5,
            },
            Entry {
                path: "zero.bin".into(),
                offset: 0,
                size: 0,
            },
            Entry {
                path: "unlisted.bin".into(),
                offset: BASE,
                size: 4,
            },
        ];
        let files = entries
            .into_iter()
            .map(|entry| (entry.path.to_ascii_lowercase(), entry))
            .collect();
        let manifest = format!(
            "{} {} Data/pattern.bin\n{} 5 Data/small.bin\n{} 0 zero.bin\n",
            hex::encode(Sha256::digest(&data)),
            data.len(),
            hex::encode(Sha256::digest(b"small")),
            hex::encode(Sha256::digest([]))
        );
        let disc = Disc {
            file: File::open(image.path()).unwrap(),
            length,
            base: 0,
            files,
        };
        (image, disc, manifest, data)
    }
    #[test]
    fn independent_stream_positions_bounds_and_ownership_retain_budget() {
        let (_image, disc, manifest, data) = fixture();
        let budget = Budget::new(INDEX_MEMORY + 2 * STREAM_MEMORY);
        let media = VerifiedDisc::prepare(disc, &manifest, &budget).unwrap();
        assert_eq!(budget.used(), INDEX_MEMORY);
        let mut first = media.open_stream("DATA\\PATTERN.BIN").unwrap();
        let mut second = media.open_stream("Data/pattern.bin").unwrap();
        assert!(media.open_stream("Data/small.bin").is_err());
        assert_eq!(budget.used(), budget.limit());
        let mut bytes = [0; 32];
        first.read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, data[..32]);
        second.seek(SeekFrom::Start(BLOCK as u64 - 8)).unwrap();
        second.read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, data[BLOCK - 8..BLOCK + 24]);
        assert_eq!(first.position(), 32);
        assert!(first.seek(SeekFrom::Current(-33)).is_err());
        assert!(first.seek(SeekFrom::Start(u64::MAX)).is_err());
        assert!(first.seek(SeekFrom::End(1)).is_err());
        assert_eq!(first.position(), 32);
        first.seek(SeekFrom::End(-13)).unwrap();
        bytes.fill(0xa5);
        assert_eq!(first.read(&mut bytes).unwrap(), 13);
        assert_eq!(&bytes[..13], &data[data.len() - 13..]);
        assert_eq!(&bytes[13..], &[0xa5; 19]);
        bytes.fill(0xa5);
        assert_eq!(first.read(&mut bytes).unwrap(), 0);
        assert_eq!(bytes, [0xa5; 32]);
        drop(media);
        assert_eq!(budget.used(), budget.limit());
        second.seek(SeekFrom::Start(0)).unwrap();
        second.read_exact(&mut bytes).unwrap();
        assert_eq!(bytes, data[..32]);
        drop(first);
        assert_eq!(budget.used(), INDEX_MEMORY + STREAM_MEMORY);
        drop(second);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn same_size_same_timestamp_tampering_and_truncation_fail_atomically() {
        let (mut image, disc, manifest, data) = fixture();
        let media = VerifiedDisc::prepare(disc, &manifest, &Budget::new(8 * 1024 * 1024)).unwrap();
        let mut stream = media.open_stream("Data/pattern.bin").unwrap();
        let start = BLOCK as u64 - 8;
        stream.seek(SeekFrom::Start(start)).unwrap();
        let modified = image.as_file().metadata().unwrap().modified().unwrap();
        image
            .seek(SeekFrom::Start(BASE + BLOCK as u64 + 1))
            .unwrap();
        image.write_all(&[data[BLOCK + 1] ^ 1]).unwrap();
        image.flush().unwrap();
        image
            .as_file()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let mut output = [0xa5; 32];
        assert!(stream.read(&mut output).is_err());
        assert_eq!(stream.position(), start);
        assert_eq!(output, [0xa5; 32]);
        image
            .seek(SeekFrom::Start(BASE + BLOCK as u64 + 1))
            .unwrap();
        image.write_all(&[data[BLOCK + 1]]).unwrap();
        image.flush().unwrap();
        stream.read_exact(&mut output).unwrap();
        assert_eq!(output, data[BLOCK - 8..BLOCK + 24]);
        stream.seek(SeekFrom::Start(start)).unwrap();
        image.as_file().set_len(BASE + BLOCK as u64 + 2).unwrap();
        // Warm pages still hold original admitted bytes. A fresh reader must
        // fail on the truncated backing data, while the warm reader stays safe.
        stream.read_exact(&mut output).unwrap();
        assert_eq!(output, data[BLOCK - 8..BLOCK + 24]);
        let mut stream = media.open_stream("Data/pattern.bin").unwrap();
        stream.seek(SeekFrom::Start(start)).unwrap();
        output.fill(0xa5);
        assert!(stream.read(&mut output).is_err());
        assert_eq!(stream.position(), start);
        assert_eq!(output, [0xa5; 32]);
    }
    #[test]
    fn concurrent_reads_use_positional_io_without_shared_cursor_interference() {
        let (_image, disc, manifest, data) = fixture();
        let budget = Budget::new(INDEX_MEMORY + 4 * STREAM_MEMORY);
        let media = VerifiedDisc::prepare(disc, &manifest, &budget).unwrap();
        let data = Arc::new(data);
        let workers = (0..4)
            .map(|seed| {
                let mut stream = media.open_stream("Data/pattern.bin").unwrap();
                let data = data.clone();
                std::thread::spawn(move || {
                    let mut output = [0; 257];
                    for iteration in 0..200 {
                        let offset = (iteration * 541 + seed * 997) % (data.len() - output.len());
                        stream.seek(SeekFrom::Start(offset as u64)).unwrap();
                        stream.read_exact(&mut output).unwrap();
                        assert_eq!(output, data[offset..offset + output.len()]);
                    }
                })
            })
            .collect::<Vec<_>>();
        drop(media);
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn every_cache_page_transition_preserves_original_bytes_and_checks_evicted_pages() {
        let (mut image, disc, manifest, data) = fixture();
        let media = VerifiedDisc::prepare(disc, &manifest, &Budget::new(8 * 1024 * 1024)).unwrap();
        let mut stream = media.open_stream("Data/pattern.bin").unwrap();
        let offsets = [0, BLOCK - 8, BLOCK + 3, 2 * BLOCK - 8, 2 * BLOCK];
        let mut output = [0; 11];
        for previous in offsets {
            for target in offsets {
                for offset in [previous, target] {
                    stream.seek(SeekFrom::Start(offset as u64)).unwrap();
                    stream.read_exact(&mut output).unwrap();
                    assert_eq!(output, data[offset..offset + 11]);
                }
            }
        }
        // Both cached pages are now1/2. A newly corrupted page0 cannot enter
        // either cache slot, and the failed miss leaves caller state untouched.
        image.seek(SeekFrom::Start(BASE)).unwrap();
        image.write_all(&[data[0] ^ 1]).unwrap();
        image.flush().unwrap();
        stream.seek(SeekFrom::Start(0)).unwrap();
        output.fill(0xa5);
        assert!(stream.read(&mut output).is_err());
        assert_eq!(stream.position(), 0);
        assert_eq!(output, [0xa5; 11]);
    }
    #[test]
    fn only_manifest_files_and_inferred_directories_are_visible() {
        let (_image, disc, manifest, _) = fixture();
        let media = VerifiedDisc::prepare(disc, &manifest, &Budget::new(8 * 1024 * 1024)).unwrap();
        let root = media.directory("").unwrap();
        assert_eq!(root.len(), 2);
        assert_eq!(root[0].name, "Data");
        assert!(root[0].is_directory);
        assert_eq!(root[1].name, "zero.bin");
        assert_eq!(media.directory("dAtA").unwrap().len(), 2);
        for path in [
            "../Data",
            "/Data",
            "\\Data",
            "Data/../zero.bin",
            "D:\\Data",
            "Data//pattern.bin",
            "Data\0",
        ] {
            assert!(media.directory(path).is_err(), "{path}");
            assert!(media.open_stream(path).is_err(), "{path}");
        }
        assert!(media.open_stream("unlisted.bin").is_err());
        assert!(media.open_stream("").is_err());
        assert!(media.open_stream("Data").is_err());
        let mut zero = media.open_stream("zero.bin").unwrap();
        assert!(zero.is_empty());
        assert_eq!(zero.read(&mut [0; 4]).unwrap(), 0);
    }
    #[test]
    fn failed_admission_and_hash_mismatch_release_every_reservation() {
        let tiny = Budget::new(OPEN_MEMORY - 1);
        assert!(VerifiedDisc::open(Path::new("missing.iso"), &tiny).is_err());
        assert_eq!(tiny.used(), 0);
        let (_image, disc, manifest, _) = fixture();
        let budget = Budget::new(8 * 1024 * 1024);
        let bad = format!(
            "{}{}",
            if &manifest[..1] == "0" { "1" } else { "0" },
            &manifest[1..]
        );
        assert!(VerifiedDisc::prepare(disc, &bad, &budget).is_err());
        assert_eq!(budget.used(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn replacing_the_selected_path_cannot_replace_an_owned_open_image() {
        let (image, disc, manifest, data) = fixture();
        let media = VerifiedDisc::prepare(disc, &manifest, &Budget::new(8 * 1024 * 1024)).unwrap();
        let replacement = tempfile::NamedTempFile::new().unwrap();
        std::fs::rename(replacement.path(), image.path()).unwrap();
        let mut stream = media.open_stream("Data/pattern.bin").unwrap();
        let mut output = [0; 32];
        stream.read_exact(&mut output).unwrap();
        assert_eq!(output, data[..32]);
    }
}
