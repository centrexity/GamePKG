//! Portable bundles retain each package's signature and deduplicate shared blobs.
use crate::{
    archive::Package,
    manifest::Manifest,
    pool::{Pool, Reference},
};
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Header {
    format_version: u32,
}
pub struct Bundle {
    _directory: tempfile::TempDir,
    games: Vec<Manifest>,
    packages: BTreeMap<String, PathBuf>,
    blobs: BTreeMap<String, (Reference, PathBuf)>,
}
fn snapshot(from: &Path, to: &Path, limit: u64) -> Result<()> {
    let input = fs::File::open(from)?;
    if !input.metadata()?.is_file() {
        bail!("store input must be a regular file");
    }
    let mut output = fs::File::create(to)?;
    if std::io::copy(&mut input.take(limit + 1), &mut output)? > limit {
        bail!("store input exceeds size limit");
    }
    Ok(())
}
impl Bundle {
    pub fn games(&self) -> &[Manifest] {
        &self.games
    }
    pub fn open(path: &Path) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        let archive_path = directory.path().join("bundle.zip");
        snapshot(path, &archive_path, crate::archive::MAX_TOTAL)?;
        let mut archive = Package::open(&archive_path)?;
        let header: Header = toml::from_str(std::str::from_utf8(&archive.read("store.toml")?)?)?;
        if header.format_version != 1 {
            bail!("unsupported store bundle version");
        }
        let mut games = Vec::new();
        let mut packages = BTreeMap::new();
        let mut expected = BTreeMap::<String, Reference>::new();
        let names: Vec<_> = archive.entries.keys().cloned().collect();
        for name in &names {
            if let Some(id) = name
                .strip_prefix("packages/")
                .and_then(|name| name.strip_suffix(".gamepkg"))
            {
                let bytes = archive.read(name)?;
                let path = directory
                    .path()
                    .join(format!("game-{}.gamepkg", games.len()));
                fs::write(&path, bytes)?;
                let manifest = crate::verify::verify_embedded(&mut Package::open(&path)?)?;
                if manifest.id != id || packages.contains_key(id) {
                    bail!("store package identity mismatch or duplicate game");
                }
                for reference in manifest.shared_assets.values() {
                    if expected
                        .insert(reference.sha256.clone(), reference.clone())
                        .is_some_and(|old| old != *reference)
                    {
                        bail!("store blob has conflicting sizes");
                    }
                }
                packages.insert(id.to_owned(), path);
                games.push(manifest);
            } else if name != "store.toml" && !name.starts_with("blobs/") {
                bail!("unexpected store bundle file");
            }
        }
        if games.is_empty() {
            bail!("store bundle contains no games");
        }
        let mut blobs = BTreeMap::new();
        for name in names.iter().filter(|name| name.starts_with("blobs/")) {
            let digest = name
                .strip_prefix("blobs/")
                .unwrap()
                .strip_suffix(".blob")
                .context("invalid store blob filename")?;
            let reference = expected.get(digest).context("unreferenced store blob")?;
            let bytes = archive.read(name)?;
            if bytes.len() as u64 != reference.size
                || crate::verify::digest(&bytes) != reference.sha256
            {
                bail!("store blob does not match signed reference");
            }
            let path = directory.path().join(format!("{digest}.blob"));
            fs::write(&path, bytes)?;
            blobs.insert(digest.to_owned(), (reference.clone(), path));
        }
        if expected.len() != blobs.len() {
            bail!("store bundle is missing a referenced blob");
        }
        Ok(Self {
            _directory: directory,
            games,
            packages,
            blobs,
        })
    }
    pub fn install(&self, root: &Path, approve_creators: bool) -> Result<usize> {
        // Preflight the whole batch before publishing any pool data or games.
        let installed = crate::registry::list(root)?;
        let mut pending = Vec::new();
        for game in &self.games {
            let path = &self.packages[&game.id];
            if let Some(old) = installed.iter().find(|old| old.id == game.id) {
                if old.creator_key != game.creator_key {
                    bail!("store update creator differs for {}", game.id);
                }
                let existing = root.join(format!("{}.gamepkg", game.id));
                if fs::read(existing)? == fs::read(path)? {
                    continue;
                }
                if semver::Version::parse(&game.version)? <= semver::Version::parse(&old.version)? {
                    bail!("store update must have a higher version for {}", game.id);
                }
            } else if !approve_creators {
                bail!(
                    "store import requires creator approval for {} ({})",
                    game.id,
                    crate::verify::digest(&hex::decode(&game.creator_key)?)
                );
            }
            pending.push(path);
        }
        let pool = Pool::open(root)?;
        for (_, path) in self.blobs.values() {
            pool.put(&fs::read(path)?)?;
        }
        for path in &pending {
            crate::registry::install(root, path, approve_creators)?;
        }
        Ok(pending.len())
    }
}
pub fn export(root: &Path, inputs: &[PathBuf], output: &Path) -> Result<()> {
    if inputs.len() > 10000 {
        bail!("too many store package inputs");
    }
    if inputs.is_empty() {
        bail!("store export needs at least one package");
    }
    let stage = tempfile::tempdir()?;
    let pool = Pool::open(root)?;
    let mut packages = BTreeMap::new();
    let mut references = BTreeMap::<String, Reference>::new();
    let mut total = 19u64;
    for (index, input) in inputs.iter().enumerate() {
        let path = stage.path().join(format!("{index}.gamepkg"));
        snapshot(input, &path, crate::archive::MAX_FILE)?;
        total += fs::metadata(&path)?.len();
        if total > crate::archive::MAX_TOTAL {
            bail!("store package contents exceed 256 MiB");
        }
        let manifest = crate::verify::verify_embedded(&mut Package::open(&path)?)?;
        if packages.insert(manifest.id, path).is_some() {
            bail!("duplicate store game ID");
        }
        for reference in manifest.shared_assets.into_values() {
            if references
                .insert(reference.sha256.clone(), reference.clone())
                .is_some_and(|old| old != reference)
            {
                bail!("store blob has conflicting sizes");
            }
        }
    }
    total += references
        .values()
        .map(|reference| reference.size)
        .sum::<u64>();
    if total > crate::archive::MAX_TOTAL || packages.len() + references.len() + 1 > 10000 {
        bail!("store bundle exceeds size or entry limit");
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut zip = zip::ZipWriter::new(temp.as_file_mut());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("store.toml", options)?;
        zip.write_all(b"format_version = 1\n")?;
        for (id, path) in packages {
            zip.start_file(format!("packages/{id}.gamepkg"), options)?;
            zip.write_all(&fs::read(path)?)?;
        }
        for (digest, reference) in references {
            zip.start_file(format!("blobs/{digest}.blob"), options)?;
            zip.write_all(&pool.read(&reference)?)?;
        }
        zip.finish()?;
    }
    temp.as_file().sync_all()?;
    if temp.as_file().metadata()?.len() > crate::archive::MAX_TOTAL {
        bail!("encoded store bundle exceeds 256 MiB");
    }
    Bundle::open(temp.path())?;
    temp.persist_noclobber(output)
        .map_err(|error| error.error)?;
    Ok(())
}
