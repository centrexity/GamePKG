//! Measured file bytes, excluding external compilers and user ROMs.
use anyhow::{bail, Result};
use std::path::Path;
#[derive(Default, Clone)]
pub struct Usage {
    pub package_bytes: u64,
    pub built_bytes: u64,
    pub estimated_bytes: Option<u64>,
}
pub fn bytes(value: u64) -> String {
    if value < 1024 {
        format!("{value} B")
    } else if value < 1048576 {
        format!("{:.1} KiB", value as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", value as f64 / 1048576.0)
    }
}
pub fn total(root: &Path) -> Result<u64> {
    fn walk(path: &Path, depth: usize, count: &mut usize) -> Result<u64> {
        if !path.exists() {
            return Ok(0);
        }
        let metadata = path.symlink_metadata()?;
        if metadata.file_type().is_symlink() {
            return Ok(0);
        }
        *count += 1;
        if *count > 100000 || depth > 16 {
            bail!("storage inventory exceeds limits");
        }
        if metadata.is_file() {
            return Ok(metadata.len());
        }
        let mut size = 0u64;
        if metadata.is_dir() {
            for item in std::fs::read_dir(path)? {
                size = size.saturating_add(walk(&item?.path(), depth + 1, count)?);
            }
        }
        Ok(size)
    }
    walk(root, 0, &mut 0)
}
pub fn usage(root: &Path, manifest: &crate::manifest::Manifest) -> Result<Usage> {
    let package = root.join(format!("{}.gamepkg", manifest.id));
    let package_bytes = std::fs::metadata(&package)?.len();
    let archive = std::fs::read(&package)?;
    let build = root
        .join("builds")
        .join(crate::verify::digest(&archive))
        .join("game.wasm");
    let built_bytes = total(build.parent().unwrap())?;
    Ok(Usage {
        package_bytes,
        built_bytes,
        estimated_bytes: manifest
            .limits
            .recommended_storage_bytes
            .map(|v| v.max(package_bytes)),
    })
}
