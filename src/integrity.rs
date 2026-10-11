//! Host-side verification and rebuild repair. Never modify signed package contents.
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

pub(crate) fn retain_package(
    root: &Path,
    manifest: &crate::manifest::Manifest,
    source: &Path,
) -> Result<()> {
    let folder = root.join("verified-packages");
    if folder
        .symlink_metadata()
        .is_ok_and(|m| !m.is_dir() || m.file_type().is_symlink())
    {
        bail!("recovery folder must be a real directory");
    }
    fs::create_dir_all(&folder)?;
    let mut copy = tempfile::NamedTempFile::new_in(&folder)?;
    let size = std::io::copy(
        &mut crate::files::open_regular(source, crate::archive::MAX_TOTAL, false)?
            .take(crate::archive::MAX_TOTAL + 1),
        &mut copy,
    )?;
    if size > crate::archive::MAX_TOTAL {
        bail!("recovery package exceeds size limit");
    }
    copy.flush()?;
    let checked = crate::verify::verify_embedded(&mut crate::archive::Package::open(copy.path())?)?;
    if checked.id != manifest.id
        || checked.creator_key != manifest.creator_key
        || checked.version != manifest.version
    {
        bail!("recovery source changed");
    }
    copy.as_file().sync_all()?;
    copy.persist(folder.join(format!("{}.gamepkg", manifest.id)))
        .map_err(|e| e.error)?;
    Ok(())
}
pub fn record_build(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("invalid build path")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(crate::verify::digest(bytes).as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path.with_extension("sha256"))
        .map_err(|e| e.error)?;
    Ok(())
}
pub fn verify_build(path: &Path, bytes: &[u8]) -> Result<()> {
    let receipt = path.with_extension("sha256");
    let expected = String::from_utf8(crate::files::read_regular(&receipt, 64, false)?)?;
    if expected.len() != 64 || expected != crate::verify::digest(bytes) {
        bail!("compiled game integrity mismatch; rebuild from verified source");
    }
    Ok(())
}
pub fn check(root: &Path, id: &str) -> Result<String> {
    let _lock = crate::registry::LibraryChange::acquire(root)?;
    check_locked(root, id)
}
fn check_locked(root: &Path, id: &str) -> Result<String> {
    // Validate the identifier through the verified registry before constructing paths.
    let manifest = crate::registry::installed_manifest(root, id)?;
    let _build_read = crate::build::BuildChange::read(root, id)?;
    let mut package = crate::archive::Package::open(&root.join(format!("{id}.gamepkg")))?;
    crate::verify::verify_embedded(&mut package)?;
    package.resolve_shared_assets(root)?;
    retain_package(root, &manifest, &root.join(format!("{id}.gamepkg")))?;
    crate::progress::load(root, &manifest.id, &manifest.creator_key)?;
    let path = crate::build::cache_path(root, id)?;
    if !path.exists() {
        return Ok(format!(
            "{id}: signed package, shared assets and play history verified; not built"
        ));
    }
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.len() > 32 * 1024 * 1024 {
        bail!("invalid compiled game file");
    }
    let bytes = crate::files::read_regular(&path, 32 * 1024 * 1024, false)?;
    verify_build(&path, &bytes)?;
    if !crate::build::is_built(root, id)? {
        bail!("compiled game has no valid playable entry point");
    }
    Ok(format!(
        "{id}: signed package, shared assets, play history and compiled game verified"
    ))
}
pub fn fix(root: &Path, id: &str) -> Result<String> {
    fix_with_store(root, id, None)
}
pub fn fix_with_store(root: &Path, id: &str, store: Option<&Path>) -> Result<String> {
    let _lock = crate::registry::LibraryChange::acquire(root)?;
    // Compilation rechecks signatures and shared blob hashes before running the isolated compiler.
    // A supplied store can restore exact signed shared assets before rebuilding.
    let store = store.map(crate::store::Bundle::open).transpose()?;
    let mut restored = 0;
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        bail!("invalid game identifier");
    }
    let target = root.join(format!("{id}.gamepkg"));
    if crate::archive::Package::open(&target)
        .and_then(|mut p| crate::verify::verify_embedded(&mut p))
        .is_err()
    {
        let backup = root.join("verified-packages").join(format!("{id}.gamepkg"));
        if fs::symlink_metadata(backup.parent().unwrap())?
            .file_type()
            .is_symlink()
            || fs::symlink_metadata(&backup)?.file_type().is_symlink()
        {
            bail!("recovery copy cannot be a symlink");
        }
        let mut file = tempfile::NamedTempFile::new_in(root)?;
        let size = std::io::copy(
            &mut crate::files::open_regular(&backup, crate::archive::MAX_TOTAL, false)?
                .take(crate::archive::MAX_TOTAL + 1),
            &mut file,
        )?;
        if size > crate::archive::MAX_TOTAL {
            bail!("recovery package exceeds size limit");
        }
        file.flush()?;
        let mut package = crate::archive::Package::open(file.path())?;
        let manifest = crate::verify::verify_embedded(&mut package)?;
        if manifest.id != id {
            bail!("recovery package ID differs");
        }
        if let Some(store) = &store {
            restored = store.repair_assets(root, &mut package)?;
        }
        package.resolve_shared_assets(root)?;
        file.as_file().sync_all()?;
        file.persist(target).map_err(|e| e.error)?;
    } else if let Some(store) = &store {
        let mut package = crate::archive::Package::open(&target)?;
        restored = store.repair_assets(root, &mut package)?;
    }
    crate::build::compile(root, id)?;
    let result = check_locked(root, id)?;
    Ok(if store.is_some() {
        format!("{result}; restored {restored} shared assets")
    } else {
        result
    })
}
#[cfg(test)]
mod tests {
    #[test]
    fn changed_artifacts_and_missing_receipts_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.wasm");
        assert!(super::verify_build(&path, b"original").is_err());
        super::record_build(&path, b"original").unwrap();
        super::verify_build(&path, b"original").unwrap();
        assert!(super::verify_build(&path, b"changed").is_err());
        std::fs::write(path.with_extension("sha256"), b"invalid").unwrap();
        assert!(super::verify_build(&path, b"original").is_err());
    }
}
