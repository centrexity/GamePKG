//! Host-owned session/process integration. Future reviewed port recipes use
//! this path; a legacy profile cannot construct its checked Artifact.
use super::{
    broker,
    preparation::{Artifact, ExecutableReceipt},
    process::Process,
    Access, Backend, Session,
};
use anyhow::{ensure, Context, Result};
use cap_fs_ext::{DirExt, FollowSymlinks, MetadataExt, OpenOptionsFollowExt};
use cap_std::fs::Dir;
use std::{fs::File, path::Path};

pub struct SaveGrant {
    directory: Dir,
    previous: Option<Dir>,
    _lock: File,
    identity: String,
    quota: u64,
}
pub(super) fn identity(manifest: &crate::manifest::Manifest) -> String {
    crate::verify::digest(
        format!(
            "{}\0{}\0{}",
            manifest.id, manifest.creator_key, manifest.save_schema
        )
        .as_bytes(),
    )
}
fn subdir(parent: &Dir, name: &str) -> Result<Dir> {
    let builder = cap_std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use cap_std::fs::DirBuilderExt;
        let mut builder = builder;
        builder.mode(0o700);
        create_subdir(parent, name, &builder)
    }
    #[cfg(not(unix))]
    create_subdir(parent, name, &builder)
}
fn create_subdir(parent: &Dir, name: &str, builder: &cap_std::fs::DirBuilder) -> Result<Dir> {
    parent.create_dir_with(name, builder).or_else(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            Ok(())
        } else {
            Err(e)
        }
    })?;
    Ok(parent.open_dir_nofollow(name)?)
}
impl SaveGrant {
    /// Private per-title/creator/schema files and a sibling host-only lock.
    /// Never use this persistent grant as a disposable map cache.
    pub fn open(root: &Path, manifest: &crate::manifest::Manifest, quota: u64) -> Result<Self> {
        let scope = save_scope(root, manifest)?;
        Self::in_scope(scope, manifest, quota)
    }

    /// Trusted bootstrap only. The launcher mounts a capability to this
    /// game's scope at a fixed location inside the native namespace.
    #[cfg(target_os = "linux")]
    pub fn open_native_mount(manifest: &crate::manifest::Manifest, quota: u64) -> Result<Self> {
        ensure!(
            manifest.permissions.save,
            "game did not request save access"
        );
        let scope =
            Dir::open_ambient_dir(Path::new("/run/gamepkg-save"), cap_std::ambient_authority())?;
        let mut grant = Self::in_scope(scope, manifest, quota)?;
        match Dir::open_ambient_dir(
            Path::new("/run/gamepkg-previous-save"),
            cap_std::ambient_authority(),
        ) {
            Ok(previous) => grant.previous = Some(previous),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(grant)
    }

    /// Trusted Windows bootstrap. The launcher grants only the scoped
    /// directory for this signed game, creator and save schema.
    #[cfg(all(windows, feature = "gui"))]
    pub fn open_native_windows(manifest: &crate::manifest::Manifest, scope: &Path) -> Result<Self> {
        ensure!(
            scope.is_absolute()
                && scope
                    .file_name()
                    .is_some_and(|name| name == identity(manifest).as_str()),
            "invalid Windows native save scope"
        );
        use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt};
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        };
        // Open the granted directory itself; the AppContainer does not have
        // list/read permission on the containing library.
        let directory = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(scope)?;
        let metadata = directory.metadata()?;
        ensure!(
            metadata.is_dir() && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0,
            "Windows native save scope must be an ordinary directory"
        );
        let mut grant = Self::in_scope(
            Dir::from_std_file(directory),
            manifest,
            manifest.native_save_quota(),
        )?;
        let exe = std::env::current_exe()?;
        let runtime = Dir::open_ambient_dir(
            exe.parent().context("missing Windows runtime directory")?,
            cap_std::ambient_authority(),
        )?;
        match runtime.open_dir_nofollow("previous-save") {
            Ok(previous) => grant.previous = Some(previous),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(grant)
    }

    fn in_scope(scope: Dir, manifest: &crate::manifest::Manifest, quota: u64) -> Result<Self> {
        ensure!(
            manifest.permissions.save,
            "game did not request save access"
        );
        let quota = manifest
            .limits
            .hard_save_bytes
            .map_or(quota, |limit| quota.min(limit));
        let mut options = cap_std::fs::OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .follow(FollowSymlinks::No);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(windows)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let lock = scope.open_with("owner.lock", &options)?;
        let metadata = lock.metadata()?;
        ensure!(
            metadata.is_file() && MetadataExt::nlink(&metadata) == 1,
            "invalid private save lock"
        );
        let lock = lock.into_std();
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            ensure!(
                unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
                "this save scope is already in use"
            );
        }
        let directory = subdir(&scope, "files")?;
        Ok(Self {
            directory,
            previous: None,
            _lock: lock,
            identity: identity(manifest),
            quota,
        })
    }
}

pub(super) fn save_scope(root: &Path, manifest: &crate::manifest::Manifest) -> Result<Dir> {
    ensure!(
        manifest.permissions.save,
        "game did not request save access"
    );
    let root = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    let parent = subdir(&root, "port-state")?;
    subdir(&parent, &identity(manifest))
}
#[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
/// Private copies of assets from a verified source package. The source identity
/// must also match the executable's audited receipt before launch.
pub struct VerifiedAssets {
    directory: tempfile::TempDir,
    identity: String,
    recorded_audio: bool,
}

#[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
impl VerifiedAssets {
    pub fn stage(
        package: &mut crate::archive::Package,
        source_identity: &str,
    ) -> Result<Option<Self>> {
        // The caller obtained this package from complete strict verification;
        // re-use that proven identity instead of scanning identical source again.
        let identity = hex::encode(
            package
                .verified_identity()
                .context("asset staging requires a package verified with its tree identity")?,
        );
        ensure!(
            identity == source_identity,
            "assets differ from the audited source kit"
        );
        let entries: Vec<_> = package
            .entries
            .keys()
            .filter_map(|path| path.strip_prefix("assets/").map(str::to_owned))
            .collect();
        let manifest = crate::manifest::Manifest::parse(&package.read("manifest.toml")?)?;
        let recorded_audio = manifest.native_tick
            && (manifest.limits.effects_memory_bytes.is_some()
                || manifest.limits.music_memory_bytes.is_some());
        if entries.is_empty() && !recorded_audio {
            return Ok(None);
        }
        let directory = tempfile::tempdir()?;
        let assets = directory.path().join("assets");
        std::fs::create_dir(&assets)?;
        if recorded_audio {
            let archive_path = directory.path().join("audio.gamepkg");
            let expected = package.archive_digest()?;
            package.copy_verified_archive(File::create(&archive_path)?)?;
            // The frozen archive is a byte-for-byte copy of the package that
            // was strictly verified above; compare bytes instead of scanning
            // the same source a second time on the launch path.
            ensure!(
                crate::files::hash_archive(&archive_path)? == expected,
                "recorded audio differs from the audited source kit"
            );
        }
        for name in entries {
            ensure!(crate::archive::valid_path(&name), "invalid asset path");
            let path = assets.join(&name);
            std::fs::create_dir_all(path.parent().context("asset parent")?)?;
            package.copy_entry(&format!("assets/{name}"), File::create(path)?)?;
        }
        Ok(Some(Self {
            directory,
            identity,
            recorded_audio,
        }))
    }

    #[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
    pub(super) fn verified_directory(&self, source_identity: &str) -> Result<std::path::PathBuf> {
        ensure!(
            self.identity == source_identity,
            "assets differ from the audited source kit"
        );
        Ok(self.directory.path().join("assets"))
    }

    pub(super) fn recorded_audio(&self) -> Option<std::path::PathBuf> {
        self.recorded_audio
            .then(|| self.directory.path().join("audio.gamepkg"))
    }
}

#[derive(Default)]
pub struct Grants {
    /// Linux external RAM cache. Windows keeps the cache inside its child cap.
    pub volatile_cache_bytes: Option<u64>,
    pub roms: Vec<File>,
    pub assets: Option<Dir>,
    /// User-approved directories opened by the provisioner, in selection order.
    pub game_data: Vec<Dir>,
    pub recorded_audio: Option<crate::archive::Package>,
    #[cfg(all(feature = "gui", target_os = "linux", target_arch = "x86_64"))]
    pub picker: Option<std::os::unix::net::UnixStream>,
    pub saves: Option<SaveGrant>,
}
pub struct Owner<B: Backend> {
    pub host: Session<B>,
    process: Process,
    pending: broker::PendingCalls,
    receipt: ExecutableReceipt,
}
/// Install the same capability grants in either process arrangement.
pub(super) fn session<B: Backend>(
    manifest: &crate::manifest::Manifest,
    backend: B,
    grants: Grants,
) -> Result<Session<B>> {
    ensure!(grants.roms.len() <= 16, "too many selected ROMs");
    ensure!(
        !manifest.requires_rom || !grants.roms.is_empty(),
        "this game requires user-selected ROM data"
    );
    let mut host = Session::new(manifest.permissions.clone(), backend);
    if manifest.native_tick {
        host.tick = Some(super::tick_services::Services::new(manifest)?);
    }
    if let Some(bytes) = grants.volatile_cache_bytes {
        host.grant_volatile_cache(bytes)
            .map_err(|e| anyhow::anyhow!("disposable RAM cache: {e:?}"))?;
    }
    if let Some(save) = grants.saves {
        ensure!(
            save.identity == identity(manifest),
            "save grant belongs to another title/creator/schema"
        );
        host.grant_save_quota(save.directory, save.quota)
            .map_err(|e| anyhow::anyhow!("save grant: {e:?}"))?;
        if let Some(previous) = save.previous {
            host.grant_directory(previous, Access::PreviousSave)
                .map_err(|e| anyhow::anyhow!("previous save grant: {e:?}"))?;
        }
        host.hold_lease(std::sync::Arc::new(save._lock));
    }
    if let Some(assets) = grants.assets {
        host.grant_directory(assets, Access::Assets)
            .map_err(|e| anyhow::anyhow!("asset grant: {e:?}"))?;
    }
    for directory in grants.game_data {
        host.grant_directory(directory, Access::GameData)
            .map_err(|e| anyhow::anyhow!("game-data grant: {e:?}"))?;
    }
    for rom in grants.roms {
        host.grant_rom(rom)
            .map_err(|e| anyhow::anyhow!("ROM grant: {e:?}"))?;
    }
    Ok(host)
}

impl<B: Backend> Owner<B> {
    pub fn start(
        artifact: &Artifact,
        backend: B,
        grants: Grants,
        memory: u64,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self> {
        let manifest = artifact.manifest();
        ensure!(
            memory
                .checked_add(grants.volatile_cache_bytes.unwrap_or(0))
                .is_some_and(|n| n <= manifest.limits.hard_memory_bytes),
            "child and external cache exceed the signed hard limit"
        );
        let host = session(manifest, backend, grants)?;
        let (connection, pending) = broker::channel_with_services(host.worker_services(), wake);
        let process = Process::start(artifact, connection, memory)?;
        Ok(Self {
            host,
            process,
            pending,
            receipt: artifact.receipt().clone(),
        })
    }
    /// Nonblocking owner dispatch. File/network/flush/deadline work is serviced
    /// directly on the private worker, never in this update budget.
    pub fn dispatch(&mut self, budget: usize) -> usize {
        self.pending.dispatch(&mut self.host, budget)
    }
    pub fn poll(&mut self) -> Result<Option<u32>> {
        self.process
            .poll()
            .with_context(|| self.process.diagnostics())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.host.timing().close();
        self.process.stop()
    }
    pub fn receipt(&self) -> &ExecutableReceipt {
        &self.receipt
    }
    pub fn diagnostics(&self) -> String {
        format!(
            "{}{}",
            self.process.diagnostics(),
            self.host.diagnostic_summary()
        )
    }
}
impl<B: Backend> Drop for Owner<B> {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_save_quota_cannot_be_raised_by_the_bootstrap() {
        let library = tempfile::tempdir().unwrap();
        let base = include_str!("../../examples/minimal/manifest.toml");
        let manifest_text = format!("native_services = true\n{base}")
            .replace("save = false", "save = true")
            .replace("[limits]", "[limits]\nhard_save_bytes = 8388608");
        let manifest = crate::manifest::Manifest::parse(manifest_text.as_bytes()).unwrap();
        let declared = SaveGrant::open(library.path(), &manifest, 32 * 1048576).unwrap();
        assert_eq!(declared.quota, 8 * 1048576);
        drop(declared);
        let lower = SaveGrant::open(library.path(), &manifest, 4 * 1048576).unwrap();
        assert_eq!(lower.quota, 4 * 1048576);
    }
}
