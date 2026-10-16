//! Imported catalogs are untrusted metadata, not installed packages or signer pins.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::Path,
};
const MAX_BYTES: u64 = 4 * 1024 * 1024;
const MAX_GAMES: usize = 10000;
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub download_url: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub requires_rom: Option<bool>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub format_version: u32,
    pub games: Vec<Entry>,
}
impl Catalog {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() as u64 > MAX_BYTES {
            bail!("game list exceeds 4 MiB");
        }
        let list: Self = toml::from_str(std::str::from_utf8(data)?)?;
        if list.format_version != 1 {
            bail!("unsupported game list format version");
        }
        if list.games.len() > MAX_GAMES {
            bail!("too many catalog games");
        }
        let mut seen = std::collections::HashSet::new();
        for entry in &list.games {
            if entry.id.is_empty()
                || entry.id.len() > 200
                || entry.id == "."
                || entry.id == ".."
                || !entry
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            {
                bail!("invalid catalog game ID");
            }
            if entry.title.trim().is_empty()
                || entry.title.len() > 256
                || entry.title.chars().any(char::is_control)
            {
                bail!("invalid catalog title");
            }
            if let Some(description) = &entry.description {
                crate::manifest::validate_metadata_text(description, 4096, true, "description")?;
            }
            if !seen.insert(&entry.id) {
                bail!("duplicate catalog game ID");
            }
            crate::registry::validate_url(&entry.download_url)?;
            if let Some(v) = &entry.version {
                semver::Version::parse(v)?;
            }
        }
        Ok(list)
    }
    pub fn read(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        Self::parse(&bytes)
    }
}
pub fn entries(root: &Path) -> Result<Vec<Entry>> {
    let path = root.join("catalog.gamepkglist");
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(Catalog::read(&path)?.games)
}
pub fn import(root: &Path, path: &Path) -> Result<usize> {
    let incoming = Catalog::read(path)?;
    fs::create_dir_all(root)?;
    let lock_path = root.join(".catalog-lock");
    let handle = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&lock_path)
        .context("another catalog import is active")?;
    struct Lock {
        path: std::path::PathBuf,
        handle: Option<fs::File>,
    }
    impl Drop for Lock {
        fn drop(&mut self) {
            self.handle.take();
            let _ = fs::remove_file(&self.path);
        }
    }
    let _lock = Lock {
        path: lock_path,
        handle: Some(handle),
    };
    let mut all: BTreeMap<String, Entry> = entries(root)?
        .into_iter()
        .map(|e| (e.id.clone(), e))
        .collect();
    let mut added = 0;
    for entry in incoming.games {
        match all.get(&entry.id) {
            Some(existing) if existing == &entry => {}
            Some(_) => bail!("catalog entry conflicts with existing game: {}", entry.id),
            None => {
                all.insert(entry.id.clone(), entry);
                added += 1;
            }
        }
    }
    let list = Catalog {
        format_version: 1,
        games: all.into_values().collect(),
    };
    let bytes = toml::to_string(&list)?.into_bytes();
    Catalog::parse(&bytes)?;
    let mut staged = tempfile::NamedTempFile::new_in(root)?;
    staged.write_all(&bytes)?;
    staged.as_file().sync_all()?;
    staged
        .persist(root.join("catalog.gamepkglist"))
        .map_err(|e| e.error)?;
    Ok(added)
}
pub fn fetch(
    root: &Path,
    id: &str,
) -> Result<(crate::manifest::Manifest, tempfile::NamedTempFile)> {
    let entry = entries(root)?
        .into_iter()
        .find(|e| e.id == id)
        .context("game is not in the catalog")?;
    let file = crate::registry::download(&entry.download_url)?;
    let manifest =
        crate::verify::verify_embedded(&mut crate::archive::Package::open(file.path())?)?;
    if manifest.id != entry.id {
        bail!("downloaded package ID does not match catalog game");
    }
    Ok((manifest, file))
}
