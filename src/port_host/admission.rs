//! Host build primitives, not a new admission profile. No legacy engine is
//! enabled by creating a receipt. Finish game-only optimization before sealing;
//! link only these exact objects plus separately reviewed host runtime objects.
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Obtained only after signature, dependency and complete source admission on
/// the build path. Launch uses `verify_authenticated` and a signed receipt that
/// attests to the exact admitted package bytes.
/// Host-generated bindings must be injected AFTER this snapshot is obtained.
pub struct CheckedSources {
    manifest: crate::manifest::Manifest,
    identity: String,
    archive_digest: String,
    files: Vec<(String, Vec<u8>)>,
}
impl CheckedSources {
    pub fn verify(package: &mut crate::archive::Package) -> Result<Self> {
        Self::verify_with_policy(package, true)
    }

    /// Authenticated launch snapshot only. The signed native build receipt must
    /// be validated against this identity before any executable can run.
    pub(crate) fn verify_authenticated(package: &mut crate::archive::Package) -> Result<Self> {
        Self::verify_with_policy(package, false)
    }

    fn verify_with_policy(
        package: &mut crate::archive::Package,
        scan_source: bool,
    ) -> Result<Self> {
        let checked = if scan_source {
            crate::verify::verify_embedded_checked(package)?
        } else {
            crate::verify::verify_embedded_authenticated_checked(package)?
        };
        let manifest = checked.manifest;
        crate::build_policy::require(manifest.builtin_engine.as_deref())?;
        let identity = hex::encode(checked.tree_identity);
        let archive_digest = package.archive_digest()?;
        let mut files = Vec::new();
        for name in package
            .entries
            .keys()
            .filter(|n| n.starts_with("source/"))
            .cloned()
            .collect::<Vec<_>>()
        {
            let bytes = package.read(&name)?;
            files.push((name, bytes));
        }
        Ok(Self {
            identity,
            archive_digest,
            files,
            manifest,
        })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn archive_digest(&self) -> &str {
        &self.archive_digest
    }
    pub fn manifest(&self) -> &crate::manifest::Manifest {
        &self.manifest
    }
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectHash {
    pub name: String,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub policy: String,
    pub sdk: String,
    pub source_kit: String,
    pub compiler: String,
    pub auditor: String,
    pub target: String,
    pub shaders: Vec<String>,
    pub objects: Vec<ObjectHash>,
}
/// A private host-created copy: compiler workers must have exited and must
/// never receive this directory as a writable mount. Read-only bits alone do
/// not constrain a same-user process; process confinement is still required.
pub struct AuditedStage {
    directory: tempfile::TempDir,
    receipt: Receipt,
}
fn hash_file(path: &Path) -> Result<String> {
    Ok(crate::verify::digest(&crate::files::read_regular(
        path,
        512 * 1024 * 1024,
        false,
    )?))
}
// On 32-bit hosts COFF normalization may hold an input and a copy together.
pub(super) const MAX_WORKER_FILE_BYTES: u64 = 12 * 1024 * 1024 * 1024;
pub(super) const MAX_GAME_OBJECT_BYTES: u64 = if cfg!(target_pointer_width = "64") {
    MAX_WORKER_FILE_BYTES
} else {
    512 * 1024 * 1024
};
pub(super) fn hash_game_object(path: &Path) -> Result<String> {
    let file = crate::files::open_regular(path, MAX_GAME_OBJECT_BYTES, false)?;
    let bytes = file.metadata()?.len();
    let mut reader = std::io::Read::take(file, MAX_GAME_OBJECT_BYTES + 1);
    let hash = crate::verify::digest_reader(&mut reader)?;
    ensure!(
        bytes <= MAX_GAME_OBJECT_BYTES && MAX_GAME_OBJECT_BYTES + 1 - reader.limit() == bytes,
        "compiler object size changed during hashing"
    );
    Ok(hash)
}

impl AuditedStage {
    pub fn seal(
        sources: &CheckedSources,
        nm: &Path,
        compiler: &Path,
        target: &str,
        shaders: Vec<String>,
        objects: &[PathBuf],
    ) -> Result<Self> {
        ensure!(
            !objects.is_empty() && objects.len() <= 4096,
            "invalid game object count"
        );
        ensure!(
            !target.is_empty()
                && target.len() <= 128
                && target
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
            "invalid compiler target identity"
        );
        ensure!(
            shaders.len() <= 256
                && shaders.iter().all(|s| s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())),
            "invalid verified shader hashes"
        );
        let directory = tempfile::tempdir()?;
        let mut hashes = Vec::new();
        let mut paths = Vec::new();
        let mut remaining = MAX_WORKER_FILE_BYTES;
        for (index, object) in objects.iter().enumerate() {
            let limit = remaining.min(MAX_GAME_OBJECT_BYTES);
            let input = crate::files::open_regular(object, limit, false)?;
            let name = format!("game-{index:04}.o");
            let path = directory.path().join(&name);
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let size = std::io::copy(&mut std::io::Read::take(input, limit + 1), &mut file)?;
            ensure!(size <= limit, "object stage too large");
            remaining -= size;
            file.sync_all()?;
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)?;
            hashes.push(ObjectHash {
                name,
                sha256: hash_game_object(&path)?,
            });
            paths.push(path);
        }
        let compiler = hash_file(compiler)?;
        let auditor = hash_file(nm)?;
        super::audit::objects(nm, &paths)?;
        ensure!(
            hash_file(nm)? == auditor,
            "auditor changed during admission"
        );
        let stage = Self {
            directory,
            receipt: Receipt {
                policy: crate::build_policy::REVISION.into(),
                sdk: super::build_identity(),
                source_kit: sources.identity.clone(),
                compiler,
                auditor,
                target: target.into(),
                shaders,
                objects: hashes,
            },
        };
        stage.verify()?;
        Ok(stage)
    }
    pub fn receipt(&self) -> &Receipt {
        &self.receipt
    }
    /// Call immediately before the trusted linker, and hash/check the final
    /// binary receipt too. No game archives/libraries or post-audit LTO allowed.
    pub fn verify(&self) -> Result<()> {
        ensure!(
            self.receipt.sdk == super::build_identity()
                && self.receipt.policy == crate::build_policy::REVISION,
            "service SDK or admission policy changed"
        );
        for object in &self.receipt.objects {
            ensure!(
                hash_game_object(&self.directory.path().join(&object.name))? == object.sha256,
                "audited game object changed"
            );
        }
        Ok(())
    }
    pub fn objects(&self) -> Result<Vec<PathBuf>> {
        self.verify()?;
        Ok(self
            .receipt
            .objects
            .iter()
            .map(|o| self.directory.path().join(&o.name))
            .collect())
    }
}
