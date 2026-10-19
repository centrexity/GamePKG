//! Host-only remembered file locations. Never included in a package or compiler exchange.
use crate::manifest::Manifest;
use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};
const MAX_BYTES: u64 = 1024 * 1024;
type Selections = BTreeMap<String, Vec<String>>;
pub(crate) fn identity(manifest: &Manifest) -> Result<String> {
    Ok(crate::verify::digest(&serde_json::to_vec(&(
        &manifest.id,
        &manifest.creator_key,
        manifest.requires_rom,
        &manifest.roms,
    ))?))
}
fn read(root: &Path) -> Result<Selections> {
    let path = root.join("rom-selections.json");
    if !path.try_exists()? {
        return Ok(BTreeMap::new());
    }
    let entries: Selections =
        serde_json::from_slice(&crate::files::read_regular(&path, MAX_BYTES, false)?)?;
    if entries.len() > 1024 {
        bail!("too many remembered games");
    }
    for (key, paths) in &entries {
        if key.len() != 64
            || !key.bytes().all(|b| b.is_ascii_hexdigit())
            || paths.len() > 8
            || paths
                .iter()
                .any(|p| p.len() > 4096 || p.chars().any(char::is_control))
        {
            bail!("invalid remembered ROM selections");
        }
    }
    Ok(entries)
}
pub(crate) fn load(root: &Path, manifest: &Manifest) -> Result<Vec<String>> {
    if manifest.required_rom_count() == 0 {
        return Ok(Vec::new());
    }
    let mut paths = read(root)?.remove(&identity(manifest)?).unwrap_or_default();
    paths.resize(manifest.required_rom_count(), String::new());
    paths.truncate(manifest.required_rom_count());
    Ok(paths)
}
pub(crate) fn save(root: &Path, manifest: &Manifest, paths: &[String]) -> Result<()> {
    if paths.len() != manifest.required_rom_count()
        || paths.len() > 8
        || paths
            .iter()
            .any(|p| p.len() > 4096 || p.chars().any(char::is_control))
    {
        bail!("invalid ROM file selection");
    }
    let mut entries = read(root)?;
    let key = identity(manifest)?;
    if paths.iter().all(|p| p.trim().is_empty()) {
        entries.remove(&key);
    } else {
        let paths = paths
            .iter()
            .map(|p| {
                let path = PathBuf::from(p.trim());
                if path.as_os_str().is_empty() || path.is_absolute() {
                    Ok(p.trim().to_owned())
                } else {
                    Ok(std::env::current_dir()?
                        .join(path)
                        .to_string_lossy()
                        .into_owned())
                }
            })
            .collect::<Result<Vec<_>>>()?;
        entries.insert(key, paths);
    }
    if entries.len() > 1024 {
        bail!("too many remembered games");
    }
    let bytes = serde_json::to_vec_pretty(&entries)?;
    if bytes.len() as u64 > MAX_BYTES {
        bail!("remembered ROM selections exceed 1 MiB");
    }
    if root
        .symlink_metadata()
        .is_ok_and(|m| !m.is_dir() || m.file_type().is_symlink())
    {
        bail!("library must be a real directory");
    }
    std::fs::create_dir_all(root)?;
    let mut staged = tempfile::NamedTempFile::new_in(root)?;
    staged.write_all(&bytes)?;
    staged.as_file().sync_all()?;
    staged
        .persist(root.join("rom-selections.json"))
        .context("unable to remember ROM paths")?;
    Ok(())
}
pub(crate) fn ready(paths: &[String], count: usize) -> bool {
    paths.len() == count
        && paths.iter().all(|p| {
            !p.trim().is_empty()
                && std::fs::metadata(p.trim())
                    .is_ok_and(|m| m.is_file() && m.len() <= 64 * 1024 * 1024)
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        toml::from_str(include_str!("../ports/doom/manifest.toml")).unwrap()
    }
    #[test]
    fn selections_persist_per_game_creator_requirement_and_library() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut first = manifest();
        first.creator_key = "creator-a".into();
        let mut second = first.clone();
        second.id = "other.game".into();
        let one = a.path().join("one.wad").to_str().unwrap().to_owned();
        let two = a.path().join("two.wad").to_str().unwrap().to_owned();
        save(a.path(), &first, std::slice::from_ref(&one)).unwrap();
        save(a.path(), &second, std::slice::from_ref(&two)).unwrap();
        assert_eq!(load(a.path(), &first).unwrap(), vec![one.clone()]);
        assert_eq!(load(a.path(), &second).unwrap(), vec![two]);
        first.version = "9.0.0".into();
        assert_eq!(load(a.path(), &first).unwrap(), vec![one]);
        assert_eq!(load(b.path(), &first).unwrap(), vec![""]);
        first.creator_key = "creator-b".into();
        assert_eq!(load(a.path(), &first).unwrap(), vec![""]);
        first.creator_key = "creator-a".into();
        first.roms[0].id = "different_rom".into();
        assert_eq!(load(a.path(), &first).unwrap(), vec![""]);
    }
    #[test]
    fn multiple_files_keep_order_and_missing_files_are_not_ready() {
        let root = tempfile::tempdir().unwrap();
        let mut m = manifest();
        m.roms.push(crate::manifest::RomRequirement {
            id: "extra".into(),
            title: "Extra".into(),
        });
        let a = root.path().join("one.rom");
        let b = root.path().join("two.rom");
        std::fs::write(&a, b"one").unwrap();
        std::fs::write(&b, b"two").unwrap();
        let paths = vec![a.to_str().unwrap().into(), b.to_str().unwrap().into()];
        save(root.path(), &m, &paths).unwrap();
        assert_eq!(load(root.path(), &m).unwrap(), paths);
        assert!(ready(&paths, 2));
        std::fs::remove_file(a).unwrap();
        assert!(!ready(&paths, 2));
        assert_eq!(load(root.path(), &m).unwrap(), paths);
        assert!(save(root.path(), &m, &["bad\npath".into()]).is_err());
        std::fs::write(root.path().join("rom-selections.json"), b"invalid").unwrap();
        assert!(load(root.path(), &m).is_err());
        m.roms.clear();
        m.requires_rom = false;
        assert!(load(root.path(), &m).unwrap().is_empty());
    }
}
