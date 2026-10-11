use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
use zip::ZipArchive;

pub const MAX_FILE: u64 = 64 * 1024 * 1024;
pub const MAX_TOTAL: u64 = 256 * 1024 * 1024;
pub fn valid_path(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('\\')
        && !name.contains(':')
        && !name.starts_with('/')
        && name
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != ".." && !p.chars().any(char::is_control))
}
pub struct Package {
    archive: ZipArchive<File>,
    pub entries: BTreeMap<String, u64>,
}
impl Package {
    pub fn open(path: &Path) -> Result<Self> {
        let mut archive = ZipArchive::new(File::open(path)?)?;
        // zip's name index collapses duplicates. Count raw central records before
        // using that index, so duplicate names cannot disappear during parsing.
        let mut raw = File::open(path)?;
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
        let mut total = 0u64;
        for i in 0..archive.len() {
            let f = archive.by_index(i)?;
            if !valid_path(f.name()) || f.is_dir() || f.is_symlink() {
                bail!("invalid archive path: {}", f.name());
            }
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
        Ok(Self { archive, entries })
    }
    pub fn read(&mut self, path: &str) -> Result<Vec<u8>> {
        if !valid_path(path) {
            bail!("invalid virtual path");
        }
        let f = self.archive.by_name(path)?;
        let mut data = Vec::new();
        f.take(MAX_FILE + 1).read_to_end(&mut data)?;
        if data.len() as u64 > MAX_FILE {
            bail!("entry exceeds read limit");
        }
        Ok(data)
    }
    pub fn asset(&mut self, path: &str) -> Result<Vec<u8>> {
        if !valid_path(path) {
            bail!("invalid asset path");
        }
        self.read(&format!("assets/{path}"))
    }
}
