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
    raw: File,
    verified: bool,
    shared_refs: BTreeMap<String, crate::pool::Reference>,
    shared_files: BTreeMap<String, tempfile::NamedTempFile>,
}
impl Package {
    pub fn open(path: &Path) -> Result<Self> {
        let raw_file = File::open(path)?;
        let mut archive = ZipArchive::new(raw_file.try_clone()?)?;
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
        let f = self.archive.by_name(path)?;
        let expected = f.size();
        let mut data = Vec::new();
        f.take(expected + 1).read_to_end(&mut data)?;
        if data.len() as u64 != expected {
            bail!("entry size does not match archive metadata");
        }
        Ok(data)
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
            files.insert(reference.sha256.clone(), snapshot);
        }
        self.shared_files = files;
        Ok(())
    }
    pub(crate) fn mark_verified(&mut self) {
        self.verified = true;
    }
    pub(crate) fn asset_chunk_cost(
        &mut self,
        path: &str,
        offset: u64,
        length: usize,
    ) -> Result<u64> {
        if !self.verified || !valid_path(path) || length > 65536 {
            bail!("invalid asset range");
        }
        if let Some(reference) = self.shared_refs.get(path) {
            if !self.shared_files.contains_key(&reference.sha256) || offset > reference.size {
                bail!("shared asset unavailable or invalid offset");
            }
            return Ok((length as u64).min(reference.size - offset) + 1024);
        }
        let entry = self.archive.by_name(&format!("assets/{path}"))?;
        if offset > entry.size() {
            bail!("asset offset exceeds entry size");
        }
        let read = (length as u64).min(entry.size() - offset);
        let prefix = if entry.compression() == zip::CompressionMethod::Stored {
            0
        } else {
            offset
        };
        Ok(prefix + read + 1024)
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
            let file = self
                .shared_files
                .get_mut(&reference.sha256)
                .context("shared asset is not resolved")?
                .as_file_mut();
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
            assert_eq!(
                package.asset_chunk_cost("data", 2, 3).unwrap(),
                if method == zip::CompressionMethod::Stored {
                    1027
                } else {
                    1029
                }
            );
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
