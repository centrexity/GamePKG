//! Read-only XDVDFS inspection. Disc entries are data, never host paths.
//! The Takedawn manifest contains only public file names, lengths and hashes.
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::File,
    path::Path,
};
#[path = "xbox_disc/media.rs"]
mod media;
pub use media::{DirectoryEntry, Stream, VerifiedDisc};

const SECTOR: u64 = 2048;
const MAGIC: &[u8] = b"MICROSOFT*XBOX*MEDIA";
const BASES: [u64; 7] = [
    0, 0xfd90, 0x2080, 0x30600, 0xfd90000, 0x18300000, 0x1fb20000,
];
const MAX_TABLE: u32 = 1024 * 1024;
const MAX_ENTRIES: usize = 8192;
const MANIFEST: &str = include_str!("../engines/takedawn/ntsc-u-retail.manifest");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub offset: u64,
    pub size: u32,
}
pub struct Disc {
    file: File,
    length: u64,
    base: u64,
    files: BTreeMap<String, Entry>,
}
#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub game: &'static str,
    pub build: &'static str,
    pub manifest_id: &'static str,
    pub partition_offset: u64,
    pub verified_files: usize,
    pub verified_bytes: u64,
    pub extra_files: usize,
    pub executable_sha256: String,
}
fn extent(offset: u64, size: u64, length: u64) -> Result<()> {
    ensure!(
        offset <= length && size <= length - offset,
        "Xbox image contains a truncated extent"
    );
    Ok(())
}
fn read_at(file: &File, length: u64, offset: u64, data: &mut [u8]) -> Result<()> {
    extent(offset, data.len() as u64, length)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.read_exact_at(data, offset)?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut offset = offset;
        let mut remaining = data;
        while !remaining.is_empty() {
            match file.seek_read(remaining, offset) {
                Ok(0) => anyhow::bail!("Xbox image was truncated during a read"),
                Ok(count) => {
                    offset += count as u64;
                    remaining = &mut remaining[count..];
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
    #[cfg(not(any(unix, windows)))]
    anyhow::bail!("positional Xbox image reads are unavailable on this host");
    Ok(())
}
fn word(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
}
fn name(bytes: &[u8]) -> Result<String> {
    ensure!(
        !bytes.is_empty() && bytes != b"." && bytes != b"..",
        "invalid Xbox disc entry name"
    );
    ensure!(
        bytes.iter().all(|b| *b >= 0x20 && !b"\x7f/\\:".contains(b)),
        "unsafe Xbox disc entry name"
    );
    // Original Xbox names are Latin-1; case folding for disc lookup is ASCII.
    Ok(bytes.iter().map(|b| char::from(*b)).collect())
}
impl Disc {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).context("open Xbox disc image read-only")?;
        Self::from_file(file)
    }

    /// Index the already-open file without reopening its pathname. The reader
    /// takes ownership and performs only positional reads on this descriptor.
    pub fn from_file(file: File) -> Result<Self> {
        let metadata = file.metadata()?;
        ensure!(metadata.is_file(), "Xbox disc image must be a regular file");
        let length = metadata.len();
        let mut descriptor = [0; SECTOR as usize];
        let mut base = None;
        for candidate in BASES {
            let offset = candidate + 32 * SECTOR;
            if extent(offset, SECTOR, length).is_err() {
                continue;
            }
            read_at(&file, length, offset, &mut descriptor)?;
            if descriptor[..20] == *MAGIC && descriptor[0x7ec..] == *MAGIC {
                base = Some(candidate);
                break;
            }
        }
        let base =
            base.context("not an Xbox XISO/full disc image; select the .iso, not the .7z archive")?;
        let mut disc = Self {
            file,
            length,
            base,
            files: BTreeMap::new(),
        };
        disc.index(word(&descriptor, 0x14), word(&descriptor, 0x18))?;
        Ok(disc)
    }
    fn index(&mut self, sector: u32, size: u32) -> Result<()> {
        let mut directories = vec![(sector, size, String::new(), 0usize)];
        let mut tables = HashSet::new();
        let mut paths = HashSet::new();
        let mut entries = 0usize;
        while let Some((sector, size, prefix, depth)) = directories.pop() {
            ensure!(
                depth <= 32 && size <= MAX_TABLE,
                "Xbox directory exceeds supported bounds"
            );
            if size == 0 {
                continue;
            }
            ensure!(
                tables.insert(sector),
                "Xbox directory cycle or repeated table"
            );
            let offset = self.base + u64::from(sector) * SECTOR;
            extent(offset, u64::from(size), self.length)?;
            let mut table = vec![0; size as usize];
            read_at(&self.file, self.length, offset, &mut table)?;
            let mut occupied = vec![false; table.len().div_ceil(4)];
            let mut visited = HashSet::new();
            let mut todo = vec![0usize];
            while let Some(at) = todo.pop() {
                ensure!(
                    visited.insert(at),
                    "Xbox directory tree cycle or duplicate link"
                );
                let header = table
                    .get(at..at + 14)
                    .context("Xbox directory link is outside its table")?;
                let left = u16::from_le_bytes(header[..2].try_into().unwrap());
                if left == u16::MAX {
                    continue;
                }
                let right = u16::from_le_bytes(header[2..4].try_into().unwrap());
                let start = word(header, 4);
                let length = word(header, 8);
                let end = at + 14 + usize::from(header[13]);
                let entry_name = name(
                    table
                        .get(at + 14..end)
                        .context("truncated Xbox disc entry name")?,
                )?;
                let slots = occupied
                    .get_mut(at / 4..end.div_ceil(4))
                    .context("Xbox directory entry exceeds table")?;
                ensure!(
                    !slots.iter().any(|used| *used),
                    "overlapping Xbox directory entries"
                );
                slots.fill(true);
                let path = if prefix.is_empty() {
                    entry_name
                } else {
                    format!("{prefix}/{entry_name}")
                };
                ensure!(path.len() < 512, "Xbox disc path exceeds supported length");
                ensure!(
                    paths.insert(path.to_ascii_lowercase()),
                    "duplicate case-insensitive Xbox disc path"
                );
                entries += 1;
                ensure!(
                    entries <= MAX_ENTRIES,
                    "Xbox disc entry count exceeds supported bounds"
                );
                let offset = self.base + u64::from(start) * SECTOR;
                extent(offset, u64::from(length), self.length)?;
                if header[12] & 0x10 != 0 {
                    directories.push((start, length, path, depth + 1));
                } else {
                    self.files.insert(
                        path.to_ascii_lowercase(),
                        Entry {
                            path,
                            offset,
                            size: length,
                        },
                    );
                }
                for link in [left, right] {
                    if link != 0 {
                        todo.push(usize::from(link) * 4);
                    }
                }
            }
        }
        Ok(())
    }
    pub fn files(&self) -> impl Iterator<Item = &Entry> {
        self.files.values()
    }
    pub fn file(&self, path: &str) -> Option<&Entry> {
        self.files.get(&path.to_ascii_lowercase())
    }
    pub fn partition_offset(&self) -> u64 {
        self.base
    }
    /// Reads only within an indexed file; no path from the image reaches the host filesystem.
    pub fn read_file(&self, path: &str, offset: u64, output: &mut [u8]) -> Result<()> {
        let entry = self.file(path).context("file is missing from Xbox disc")?;
        extent(offset, output.len() as u64, u64::from(entry.size))?;
        let absolute = entry.offset + offset;
        read_at(&self.file, self.length, absolute, output)
    }
    pub fn verify_takedawn(&mut self) -> Result<Report> {
        let report = self.verify_blocks(MANIFEST, |_, _| Ok(()))?;
        ensure!(
            report.verified_files == 716 && report.verified_bytes == 2476107072,
            "pinned disc manifest is incomplete"
        );
        Ok(report)
    }
    fn verify_blocks(
        &self,
        manifest: &str,
        mut block: impl FnMut(&str, &[u8]) -> Result<()>,
    ) -> Result<Report> {
        let before = self.file.metadata()?;
        let mut buffer = [0u8; 65536];
        let mut count = 0;
        let mut total = 0;
        let mut executable_sha256 = String::new();
        for line in manifest
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let mut fields = line.splitn(3, ' ');
            let expected = fields.next().context("invalid pinned disc manifest")?;
            let size: u64 = fields
                .next()
                .context("invalid pinned disc manifest")?
                .parse()?;
            let path = fields.next().context("invalid pinned disc manifest")?;
            let entry = self
                .file(path)
                .with_context(|| format!("supported Xbox disc file is missing: {path}"))?;
            ensure!(
                u64::from(entry.size) == size,
                "unsupported or damaged disc: wrong file size for {path}"
            );
            let mut hash = Sha256::new();
            let mut offset = 0;
            while offset < size {
                let length = (size - offset).min(buffer.len() as u64) as usize;
                self.read_file(path, offset, &mut buffer[..length])?;
                hash.update(&buffer[..length]);
                block(path, &buffer[..length])?;
                offset += length as u64;
            }
            let actual = hex::encode(hash.finalize());
            ensure!(
                actual == expected,
                "unsupported or damaged disc: SHA-256 mismatch for {path}"
            );
            if path == "default.xbe" {
                executable_sha256 = actual;
            }
            count += 1;
            total += size;
        }
        let after = self.file.metadata()?;
        ensure!(
            before.len() == self.length
                && after.len() == self.length
                && before.modified()? == after.modified()?,
            "Xbox disc changed during verification"
        );
        Ok(Report {
            game: "Burnout 3: Takedown (Xbox)",
            build: "North America (NTSC-U) retail",
            manifest_id: "81dc20fd647fb0b1",
            partition_offset: self.base,
            verified_files: count,
            verified_bytes: total,
            extra_files: self.files.len() - count,
            executable_sha256,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom, Write};

    fn node(
        table: &mut [u8],
        at: usize,
        right: u16,
        sector: u32,
        size: u32,
        directory: bool,
        name: &[u8],
    ) {
        table[at..at + 2].copy_from_slice(&0u16.to_le_bytes());
        table[at + 2..at + 4].copy_from_slice(&right.to_le_bytes());
        table[at + 4..at + 8].copy_from_slice(&sector.to_le_bytes());
        table[at + 8..at + 12].copy_from_slice(&size.to_le_bytes());
        table[at + 12] = if directory { 0x10 } else { 0 };
        table[at + 13] = name.len() as u8;
        table[at + 14..at + 14 + name.len()].copy_from_slice(name);
    }
    fn fixture(base: u64) -> tempfile::NamedTempFile {
        let mut output = tempfile::NamedTempFile::new().unwrap();
        output.as_file().set_len(base + 42 * SECTOR).unwrap();
        let mut descriptor = [0; SECTOR as usize];
        descriptor[..20].copy_from_slice(MAGIC);
        descriptor[0x7ec..].copy_from_slice(MAGIC);
        descriptor[0x14..0x18].copy_from_slice(&34u32.to_le_bytes());
        descriptor[0x18..0x1c].copy_from_slice(&64u32.to_le_bytes());
        output.seek(SeekFrom::Start(base + 32 * SECTOR)).unwrap();
        output.write_all(&descriptor).unwrap();
        let mut table = [255; 64];
        node(&mut table, 0, 7, 40, 4, false, b"default.xbe");
        node(&mut table, 28, 0, 35, 64, true, b"Data");
        output.seek(SeekFrom::Start(base + 34 * SECTOR)).unwrap();
        output.write_all(&table).unwrap();
        table.fill(255);
        node(&mut table, 0, 0, 41, 3, false, b"race.bin");
        output.seek(SeekFrom::Start(base + 35 * SECTOR)).unwrap();
        output.write_all(&table).unwrap();
        for (sector, bytes) in [(40, b"XBE!".as_slice()), (41, b"car".as_slice())] {
            output
                .seek(SeekFrom::Start(base + sector * SECTOR))
                .unwrap();
            output.write_all(bytes).unwrap();
        }
        output.flush().unwrap();
        output
    }
    #[test]
    fn trimmed_and_full_images_read_in_place_with_bounded_case_insensitive_files() {
        for base in [0, 0x18300000] {
            let image = fixture(base);
            let mut disc = Disc::open(image.path()).unwrap();
            assert_eq!(disc.partition_offset(), base);
            assert_eq!(disc.files().count(), 2);
            let mut data = [0; 3];
            disc.read_file("DATA/RACE.BIN", 0, &mut data).unwrap();
            assert_eq!(&data, b"car");
            assert!(disc.read_file("Data/race.bin", 1, &mut data).is_err());
            assert!(disc.read_file("../default.xbe", 0, &mut data).is_err());
            assert!(disc.verify_takedawn().is_err());
        }
    }
    #[test]
    fn supplied_file_survives_path_replacement_and_still_checks_extents() {
        let image = fixture(0);
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("selected.iso");
        std::fs::copy(image.path(), &path).unwrap();
        let opened = File::open(&path).unwrap();
        std::fs::rename(&path, folder.path().join("renamed.iso")).unwrap();
        std::fs::write(&path, b"replacement, not a disc").unwrap();
        let disc = Disc::from_file(opened).unwrap();
        let mut data = [0; 3];
        disc.read_file("Data/race.bin", 0, &mut data).unwrap();
        assert_eq!(&data, b"car");
        assert!(disc.read_file("Data/race.bin", 1, &mut data).is_err());
        assert!(Disc::open(&path).is_err());
    }
    #[test]
    fn damaged_extents_tree_cycles_overlaps_and_unsafe_names_fail_closed() {
        for (offset, bytes) in [
            (32 * SECTOR + 0x7ec, vec![0]),
            (32 * SECTOR + 0x14, u32::MAX.to_le_bytes().to_vec()),
            (34 * SECTOR + 28 + 2, 7u16.to_le_bytes().to_vec()),
            (34 * SECTOR + 2, 1u16.to_le_bytes().to_vec()),
            (34 * SECTOR + 14, b"../evil.bin".to_vec()),
            (34 * SECTOR + 4, u32::MAX.to_le_bytes().to_vec()),
            (34 * SECTOR + 28 + 4, 34u32.to_le_bytes().to_vec()),
        ] {
            let mut image = fixture(0);
            image.seek(SeekFrom::Start(offset)).unwrap();
            image.write_all(&bytes).unwrap();
            image.flush().unwrap();
            assert!(Disc::open(image.path()).is_err(), "offset {offset}");
        }
        let mut image = fixture(0);
        let mut table = [255; 64];
        node(&mut table, 0, 7, 40, 4, false, b"default.xbe");
        node(&mut table, 28, 0, 41, 3, false, b"DEFAULT.XBE");
        image.seek(SeekFrom::Start(34 * SECTOR)).unwrap();
        image.write_all(&table).unwrap();
        image.flush().unwrap();
        assert!(Disc::open(image.path()).is_err());
    }
    #[test]
    fn pinned_supported_disc_manifest_has_all_unique_expected_files() {
        let lock: serde_json::Value =
            serde_json::from_str(include_str!("../engines/takedawn/upstream-lock.json")).unwrap();
        assert_eq!(
            hex::encode(Sha256::digest(MANIFEST.as_bytes())),
            lock["disc_manifest"]["sha256"].as_str().unwrap()
        );
        let mut names = HashSet::new();
        let mut bytes = 0u64;
        for line in MANIFEST
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let fields = line.splitn(3, ' ').collect::<Vec<_>>();
            assert_eq!(fields.len(), 3);
            assert_eq!(hex::decode(fields[0]).unwrap().len(), 32);
            assert!(names.insert(fields[2].to_ascii_lowercase()));
            bytes += fields[1].parse::<u64>().unwrap();
        }
        assert_eq!(names.len(), 716);
        assert_eq!(bytes, 2476107072);
        assert!(names.contains("default.xbe"));
    }
}
