//! Host-only remembered data folders, scoped to signed creator and declaration.
use crate::manifest::Manifest;
use anyhow::{ensure, Context, Result};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
pub(crate) fn identity(manifest: &Manifest) -> Result<String> {
    ensure!(
        manifest.game_data.is_some()
            && manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE),
        "this game does not request a supported data folder"
    );
    Ok(crate::verify::digest(&serde_json::to_vec(&(
        &manifest.id,
        &manifest.creator_key,
        &manifest.game_data,
        &manifest.builtin_engine,
    ))?))
}
fn read(root: &Path) -> Result<BTreeMap<String, String>> {
    let path = root.join("game-data-selections.json");
    if !path.try_exists()? {
        return Ok(BTreeMap::new());
    }
    let entries: BTreeMap<String, String> =
        serde_json::from_slice(&crate::files::read_regular(&path, 1048576, false)?)?;
    ensure!(
        entries.len() <= 1024
            && entries.iter().all(|(k, v)| k.len() == 64
                && k.bytes().all(|b| b.is_ascii_hexdigit())
                && v.len() <= 4096
                && !v.chars().any(char::is_control)),
        "invalid remembered game-data selections"
    );
    Ok(entries)
}
pub fn load(root: &Path, manifest: &Manifest) -> Result<Option<PathBuf>> {
    if manifest.game_data.is_none() {
        return Ok(None);
    }
    Ok(read(root)?.remove(&identity(manifest)?).map(PathBuf::from))
}
pub fn remember(root: &Path, manifest: &Manifest, path: &Path) -> Result<()> {
    let key = identity(manifest)?;
    let path = std::fs::canonicalize(path).context("choose the original game-data folder")?;
    ensure!(path.is_dir(), "game data must be a folder");
    let text = path
        .to_str()
        .context("folder location must be representable as text")?;
    ensure!(
        text.len() <= 4096 && !text.chars().any(char::is_control),
        "invalid game-data location"
    );
    for name in ["data", "packages"] {
        let metadata = path.join(name).symlink_metadata()?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "choose the original installation containing real data and packages folders"
        );
    }
    let mut entries = read(root)?;
    entries.insert(key, text.to_owned());
    ensure!(entries.len() <= 1024, "too many remembered games");
    let bytes = serde_json::to_vec_pretty(&entries)?;
    ensure!(
        bytes.len() <= 1048576,
        "remembered game-data locations exceed their quota"
    );
    if let Ok(metadata) = root.symlink_metadata() {
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "library must be a real directory"
        );
    }
    std::fs::create_dir_all(root)?;
    let mut staged = tempfile::NamedTempFile::new_in(root)?;
    staged.write_all(&bytes)?;
    staged.as_file().sync_all()?;
    staged
        .persist(root.join("game-data-selections.json"))
        .map_err(|e| e.error)?;
    Ok(())
}
pub fn remember_for_game(root: &Path, id: &str, path: &Path) -> Result<()> {
    let _change = crate::registry::LibraryChange::acquire(root)?;
    let manifest = crate::registry::installed_manifest(root, id)?;
    remember(root, &manifest, path)
}
