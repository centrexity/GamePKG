//! Logical file sizes and OS-reported file allocation, excluding external compilers and user ROMs.
mod allocated;
use anyhow::{bail, Result};
use std::path::Path;
#[derive(Default, Clone)]
pub struct Usage {
    pub package_bytes: u64,
    pub recovery_bytes: u64,
    pub built_bytes: u64,
    pub shader_bytes: u64,
    pub save_bytes: u64,
    pub history_bytes: u64,
    pub window_bytes: u64,
    pub shared_bytes: u64,
    pub estimated_bytes: Option<u64>,
    pub on_disk_bytes: Option<u64>,
}
impl Usage {
    pub fn used_bytes(&self) -> u64 {
        [
            self.package_bytes,
            self.recovery_bytes,
            self.built_bytes,
            self.shader_bytes,
            self.save_bytes,
            self.history_bytes,
            self.window_bytes,
            self.shared_bytes,
        ]
        .into_iter()
        .fold(0, u64::saturating_add)
    }
    pub fn space(&self) -> Space {
        Space {
            file_bytes: self.used_bytes(),
            on_disk_bytes: self.on_disk_bytes,
        }
    }
    pub fn breakdown(&self) -> String {
        format!("{}. File sizes by category — Package: {}; recovery copy: {}; current build and reusable compiler files: {}; shader caches: {}; saves and snapshots: {}; play history: {}; window settings: {}; cached shared assets: {}. Compiler files and shared assets may also be used by other games. Older Wasm builds and save schemas remain in the library total.",self.space().details(),bytes(self.package_bytes),bytes(self.recovery_bytes),bytes(self.built_bytes),bytes(self.shader_bytes),bytes(self.save_bytes),bytes(self.history_bytes),bytes(self.window_bytes),bytes(self.shared_bytes))
    }
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
/// File sizes remain available separately from the allocation shown to users.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub file_bytes: u64,
    pub on_disk_bytes: Option<u64>,
}
impl Default for Space {
    fn default() -> Self {
        Self {
            file_bytes: 0,
            on_disk_bytes: Some(0),
        }
    }
}
impl Space {
    fn add(&mut self, other: Self) {
        self.file_bytes = self.file_bytes.saturating_add(other.file_bytes);
        self.on_disk_bytes = self
            .on_disk_bytes
            .zip(other.on_disk_bytes)
            .map(|(a, b)| a.saturating_add(b));
    }
    pub fn display_bytes(&self) -> String {
        bytes(self.on_disk_bytes.unwrap_or(self.file_bytes))
    }
    pub fn details(&self) -> String {
        if self.on_disk_bytes.is_some() {
            format!("{}; file sizes: {}", self.label(), bytes(self.file_bytes))
        } else {
            format!("{} (disk allocation unavailable)", self.label())
        }
    }
    pub fn label(&self) -> String {
        format!(
            "{} {}",
            self.display_bytes(),
            if self.on_disk_bytes.is_some() {
                "on disk"
            } else {
                "in files"
            }
        )
    }
}
/// Compatibility: total logical file sizes, not physical allocation.
pub fn total(root: &Path) -> Result<u64> {
    Ok(space(root)?.file_bytes)
}
pub fn space(root: &Path) -> Result<Space> {
    fn walk(path: &Path, depth: usize, count: &mut usize) -> Result<Space> {
        let metadata = match path.symlink_metadata() {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Space::default()),
            Err(e) => return Err(e.into()),
        };
        if metadata.file_type().is_symlink() {
            return Ok(Space::default());
        }
        *count += 1;
        if *count > 100000 || depth > 16 {
            bail!("storage inventory exceeds limits");
        }
        if metadata.is_file() {
            return Ok(Space {
                file_bytes: metadata.len(),
                on_disk_bytes: allocated::file(path, &metadata),
            });
        }
        let mut result = Space::default();
        if metadata.is_dir() {
            for item in std::fs::read_dir(path)? {
                result.add(walk(&item?.path(), depth + 1, count)?);
            }
        }
        Ok(result)
    }
    walk(root, 0, &mut 0)
}
fn scoped_total(root: &Path, path: &Path, allocation: &mut Option<u64>) -> Result<u64> {
    let mut current = root.to_path_buf();
    for part in path.strip_prefix(root)?.components() {
        current.push(part);
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(0),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        }
    }
    let result = space(path)?;
    *allocation = allocation
        .zip(result.on_disk_bytes)
        .map(|(a, b)| a.saturating_add(b));
    Ok(result.file_bytes)
}
pub fn usage(root: &Path, manifest: &crate::manifest::Manifest) -> Result<Usage> {
    let package = root.join(format!("{}.gamepkg", manifest.id));
    let mut file = crate::files::open_regular(&package, crate::archive::MAX_TOTAL, false)?;
    let metadata = file.metadata()?;
    let package_bytes = metadata.len();
    let mut allocation = allocated::file(&package, &metadata);
    use sha2::Digest;
    use std::io::Read;

    let mut hash = sha2::Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut read_bytes = 0u64;
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        read_bytes = read_bytes.saturating_add(length as u64);
        if read_bytes > crate::archive::MAX_TOTAL {
            bail!("package exceeds storage inventory read limit");
        }
        hash.update(&buffer[..length]);
    }
    let build = root
        .join("builds")
        .join(hex::encode(hash.finalize()))
        .join("game.wasm");
    let mut measure = |path: &Path| scoped_total(root, path, &mut allocation);
    let mut built_bytes = measure(build.parent().unwrap())?;
    if manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE) {
        for cache in ["takedawn", "takedawn-windows"] {
            built_bytes =
                built_bytes.saturating_add(measure(&root.join("native-unit-cache").join(cache))?);
        }
    }
    let mut shader_bytes = 0u64;
    let shaders = root.join("shader-builds");
    if shaders.exists() && !shaders.symlink_metadata()?.file_type().is_symlink() {
        for (index, item) in std::fs::read_dir(&shaders)?.enumerate() {
            if index >= 10000 {
                bail!("too many shader caches for storage inventory");
            }
            let item = item?;
            if !item.file_type()?.is_dir() {
                continue;
            }
            let metadata = item.path().join("manifest.toml");
            if metadata
                .symlink_metadata()
                .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
            {
                let mut bytes = Vec::new();
                std::fs::File::open(metadata)?
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 1024 * 1024 {
                    continue;
                }
                if let Ok(cached) = crate::manifest::Manifest::parse(&bytes) {
                    if cached.id == manifest.id && cached.creator_key == manifest.creator_key {
                        shader_bytes = shader_bytes.saturating_add(measure(&item.path())?);
                    }
                }
            }
        }
    }
    let scope = if manifest.save_schema == 1 {
        format!("GamePKG save v1\0{}\0{}", manifest.id, manifest.creator_key)
    } else {
        format!(
            "GamePKG save v1\0{}\0{}\0schema={}",
            manifest.id, manifest.creator_key, manifest.save_schema
        )
    };
    let mut save_bytes = measure(
        &root
            .join("saves")
            .join(format!("{}.blob", crate::verify::digest(scope.as_bytes()))),
    )?;
    if manifest.builtin_engine.as_deref() == Some(crate::cube2::PROFILE) {
        save_bytes = save_bytes.saturating_add(measure(&crate::cube2::save_path(
            root,
            &manifest.id,
            &manifest.creator_key,
            manifest.save_schema,
        ))?);
    }
    if manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE) {
        save_bytes = save_bytes.saturating_add(measure(&crate::takedawn::data_path(
            root,
            &manifest.id,
            &manifest.creator_key,
            manifest.save_schema,
        ))?);
    }
    for name in crate::zelda_snapshots::slot_names(manifest)? {
        save_bytes = save_bytes.saturating_add(measure(&root.join("states").join(name))?);
    }
    let history_bytes = measure(&crate::progress::path(
        root,
        &manifest.id,
        &manifest.creator_key,
    )?)?;
    let mut shared_bytes = 0u64;
    let mut seen = std::collections::HashSet::new();
    for reference in manifest.shared_assets.values() {
        reference.validate()?;
        if seen.insert(&reference.sha256) {
            shared_bytes = shared_bytes.saturating_add(measure(
                &root
                    .join("asset-pool")
                    .join(format!("{}.blob", reference.sha256)),
            )?);
        }
    }
    Ok(Usage {
        package_bytes,
        recovery_bytes: measure(
            &root
                .join("verified-packages")
                .join(format!("{}.gamepkg", manifest.id)),
        )?,
        built_bytes,
        shader_bytes,
        save_bytes,
        history_bytes,
        window_bytes: measure(&crate::window_preferences::path(
            root,
            &manifest.id,
            &manifest.creator_key,
        ))?,
        shared_bytes,
        estimated_bytes: manifest
            .limits
            .recommended_storage_bytes
            .map(|v| v.max(package_bytes)),
        on_disk_bytes: allocation,
    })
}
