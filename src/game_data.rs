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
/// Basic startup readiness, not a complete asset/version compatibility audit.
pub fn validate(path: &Path) -> Result<()> {
    ensure!(path.is_dir(), "selected game-data folder is unavailable");
    for name in ["data", "packages"] {
        let metadata = path
            .join(name)
            .symlink_metadata()
            .with_context(|| format!("game data is missing {name}"))?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "game data requires a real {name} folder"
        );
    }
    validate_parts(&path.join("data"), &path.join("packages"))
}
pub(crate) fn validate_parts(data: &Path, packages: &Path) -> Result<()> {
    let metadata = packages
        .join("textures")
        .symlink_metadata()
        .context("game data is missing packages/textures")?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "game data requires a real packages/textures folder"
    );
    for (path, name) in [
        (data.join("stdlib.cfg"), "data/stdlib.cfg"),
        (
            packages.join("textures/notexture.png"),
            "packages/textures/notexture.png",
        ),
    ] {
        let metadata = path
            .symlink_metadata()
            .with_context(|| format!("game data is missing {name}"))?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() > 0,
            "game data requires a nonempty regular {name} file"
        );
    }
    Ok(())
}
pub fn available(path: &Path) -> bool {
    validate(path).is_ok()
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
    validate(&path)?;
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

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn linked_core_files_and_texture_directory_are_not_ready() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::create_dir_all(root.path().join("packages/textures")).unwrap();
        let target = root.path().join("original");
        std::fs::write(&target, b"original").unwrap();
        let names = ["data/stdlib.cfg", "packages/textures/notexture.png"];
        for name in names {
            std::fs::write(root.path().join(name), b"original").unwrap();
        }
        for name in names {
            let path = root.path().join(name);
            std::fs::remove_file(&path).unwrap();
            symlink(&target, &path).unwrap();
            assert!(!super::available(root.path()));
            std::fs::remove_file(&path).unwrap();
            std::fs::write(&path, b"original").unwrap();
        }
        let textures = root.path().join("packages/textures");
        let moved = root.path().join("original-textures");
        std::fs::rename(&textures, &moved).unwrap();
        symlink(&moved, &textures).unwrap();
        assert!(!super::available(root.path()));
        assert_eq!(std::fs::read(target).unwrap(), b"original");
    }
    #[test]
    fn incomplete_data_is_not_ready() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("data")).unwrap();
        std::fs::create_dir(root.path().join("packages")).unwrap();
        assert!(
            !super::available(root.path()),
            "Empty folders were marked ready to play"
        );
    }
    #[test]
    fn core_readiness_reports_each_missing_empty_or_nonregular_file() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::create_dir_all(root.path().join("packages/textures")).unwrap();
        let names = ["data/stdlib.cfg", "packages/textures/notexture.png"];
        for name in names {
            std::fs::write(root.path().join(name), b"original").unwrap();
        }
        assert!(super::available(root.path()));
        for name in names {
            let path = root.path().join(name);
            std::fs::remove_file(&path).unwrap();
            assert!(super::validate(root.path())
                .unwrap_err()
                .to_string()
                .contains(name));
            std::fs::write(&path, b"").unwrap();
            assert!(!super::available(root.path()));
            std::fs::remove_file(&path).unwrap();
            std::fs::create_dir(&path).unwrap();
            assert!(!super::available(root.path()));
            std::fs::remove_dir(&path).unwrap();
            std::fs::write(&path, b"original").unwrap();
            assert!(super::available(root.path()));
        }
    }
}
