//! Source-checked native service builds. No legacy engine, Cargo hook, game
//! library or post-audit LTO enters this path. This does not enable a profile.
use super::admission::{AuditedStage, CheckedSources, ObjectHash, Receipt, MAX_WORKER_FILE_BYTES};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(any(target_os = "linux", all(windows, feature = "gui")))]
use std::process::Command;
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
#[path = "preparation_local.rs"]
mod local;
#[cfg(all(
    feature = "gui",
    any(windows, all(target_os = "linux", target_arch = "x86_64"))
))]
pub use local::build_in_process;
#[cfg(all(feature = "gui", windows))]
pub use local::build_windows_in_process;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Target {
    Linux64,
    Linux32,
    Windows64,
    Windows32,
}
impl Target {
    pub fn triple(self) -> &'static str {
        match self {
            Self::Linux64 => "x86_64-unknown-linux-gnu",
            Self::Linux32 => "i686-unknown-linux-gnu",
            Self::Windows64 => "x86_64-pc-windows-gnu",
            Self::Windows32 => "i686-pc-windows-gnu",
        }
    }
    fn windows(self) -> bool {
        matches!(self, Self::Windows64 | Self::Windows32)
    }
    fn native(self) -> bool {
        self.windows() == cfg!(windows)
            && matches!(
                (self, std::env::consts::ARCH),
                (Self::Linux64 | Self::Windows64, "x86_64")
                    | (Self::Linux32 | Self::Windows32, "x86")
            )
    }
    fn validate(self, bytes: &[u8]) -> Result<()> {
        if self.windows() {
            return crate::native::validate_pe(
                bytes,
                if self == Self::Windows64 {
                    "x86_64"
                } else {
                    "x86"
                },
            );
        }
        ensure!(
            bytes.len() > 64 && bytes.starts_with(b"\x7fELF") && bytes[5] == 1,
            "invalid native ELF"
        );
        ensure!(
            bytes[4] == if self == Self::Linux64 { 2 } else { 1 },
            "wrong native ELF width"
        );
        ensure!(
            u16::from_le_bytes([bytes[18], bytes[19]])
                == if self == Self::Linux64 { 62 } else { 3 },
            "wrong native ELF machine"
        );
        ensure!(
            matches!(u16::from_le_bytes([bytes[16], bytes[17]]), 2 | 3),
            "ELF must be an executable"
        );
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LocalLayout {
    None,
    /// Host-owned physical RAM backing for checked, statically compiled Xbox
    /// functions. Guest addresses are offsets/aliases, never native pointers.
    NativeRam,
    /// Separate checked main/contiguous RAM and flash backings. The port
    /// supplies explicit guest aliases; device apertures are not RAM windows.
    NativeMemory,
    /// The same game RAM/flash plus a separate 1 MiB non-guest scratch window.
    /// The host selects this reviewed layout; game source cannot request bases.
    NativeServices,
    I686FixedLayout,
    CustomEditionI686,
}
impl LocalLayout {
    fn windows(self) -> &'static [super::local_runtime::Window] {
        use super::local_runtime::Window;
        const MAIN: Window = Window {
            address: 0x20000000,
            bytes: 0x04000000,
        };
        const CONTIGUOUS: Window = Window {
            address: 0x24000000,
            bytes: 0x04000000,
        };
        const FLASH: Window = Window {
            address: 0x28000000,
            bytes: 0x00100000,
        };
        const SCRATCH: Window = Window {
            address: 0x29000000,
            bytes: 0x00100000,
        };
        const GUEST_IMAGE_RAM: Window = Window {
            address: 0x80000000,
            bytes: 0x20000000,
        };
        match self {
            Self::None => &[],
            Self::NativeRam => &[MAIN],
            Self::NativeMemory => &[MAIN, CONTIGUOUS, FLASH],
            Self::NativeServices => &[MAIN, CONTIGUOUS, FLASH, SCRATCH],
            Self::I686FixedLayout => &[GUEST_IMAGE_RAM],
            Self::CustomEditionI686 => &[
                GUEST_IMAGE_RAM,
                Window {
                    address: 0x40440000,
                    bytes: 0x01700000,
                },
            ],
        }
    }
}

const DEFAULT_CODEGEN_UNITS: u32 = 64;
const MAX_CODEGEN_UNITS: u32 = 512;
const COMPILER_TIMEOUT_SECS: u64 = 10_800;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawPlan")]
pub struct Plan {
    target: Target,
    entry: String,
    tasks: Vec<String>,
    layout: LocalLayout,
    optimize: bool,
    codegen_units: u32,
    compiler_memory_bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    target: Target,
    entry: String,
    tasks: Vec<String>,
    layout: LocalLayout,
    optimize: bool,
    #[serde(default = "one_codegen_unit")]
    codegen_units: u32,
    #[serde(default = "compiler_memory_default")]
    compiler_memory_bytes: u64,
}
fn compiler_memory_default() -> u64 {
    if cfg!(all(windows, target_pointer_width = "32")) {
        2 * 1024 * 1024 * 1024
    } else {
        24 * 1024 * 1024 * 1024
    }
}
fn one_codegen_unit() -> u32 {
    1
}
impl TryFrom<RawPlan> for Plan {
    type Error = anyhow::Error;
    fn try_from(raw: RawPlan) -> Result<Self> {
        Self::new(raw.target, &raw.entry, raw.tasks, raw.layout, raw.optimize)?
            .with_codegen_units(raw.codegen_units)?
            .with_compiler_memory_bytes(raw.compiler_memory_bytes)
    }
}
impl Plan {
    /// HOST build metadata, not deserialized from a game's request/manifest.
    pub fn new(
        target: Target,
        entry: &str,
        tasks: Vec<String>,
        layout: LocalLayout,
        optimize: bool,
    ) -> Result<Self> {
        let name = |s: &str| {
            !s.is_empty()
                && s.len() <= 128
                && s.as_bytes()[0].is_ascii_alphabetic()
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && !s.starts_with("__gamepkg")
        };
        ensure!(
            name(entry) && tasks.len() <= 256 && tasks.iter().all(|s| name(s)),
            "invalid reviewed entry table"
        );
        ensure!(
            matches!(
                layout,
                LocalLayout::None
                    | LocalLayout::NativeRam
                    | LocalLayout::NativeMemory
                    | LocalLayout::NativeServices
            ) || matches!(target, Target::Linux32 | Target::Windows32),
            "fixed Xbox layout requires i686"
        );
        Ok(Self {
            target,
            entry: entry.into(),
            tasks,
            layout,
            optimize,
            codegen_units: DEFAULT_CODEGEN_UNITS,
            compiler_memory_bytes: compiler_memory_default(),
        })
    }
    /// Reviewed host choice, never a game Cargo flag or profile exemption.
    /// Each LLVM unit is separately optimized with LTO/bitcode disabled and
    /// every resulting relocatable object enters the same sealed audit stage.
    pub fn with_codegen_units(mut self, units: u32) -> Result<Self> {
        ensure!(
            (1..=MAX_CODEGEN_UNITS).contains(&units),
            "invalid reviewed codegen-unit count"
        );
        self.codegen_units = units;
        Ok(self)
    }
    /// Host-reviewed compiler budget, separate from game runtime limits. No
    /// package/creator option selects it. Bound into Plan/cache provenance.
    pub fn with_compiler_memory_bytes(mut self, bytes: u64) -> Result<Self> {
        ensure!(
            crate::compiler::SERVICE_MEMORY_BUDGET.contains(&bytes),
            "invalid reviewed compiler memory budget"
        );
        #[cfg(windows)]
        ensure!(
            usize::try_from(bytes).is_ok(),
            "compiler memory budget exceeds this host architecture"
        );
        self.compiler_memory_bytes = bytes;
        Ok(self)
    }
    pub fn target(&self) -> Target {
        self.target
    }
    pub fn identity(&self) -> String {
        crate::verify::digest(&serde_json::to_vec(self).expect("fixed plan serialization"))
    }
    fn arguments(&self) -> Vec<OsString> {
        let cpu = if self.optimize && self.target.native() {
            "native"
        } else if matches!(self.target, Target::Linux32 | Target::Windows32) {
            "pentium4"
        } else {
            "x86-64"
        };
        [
            "--edition=2021",
            "--target",
            self.target.triple(),
            "-C",
            "panic=abort",
            "-C",
            "lto=off",
            "-C",
            "embed-bitcode=no",
            "-C",
            &format!("codegen-units={}", self.codegen_units),
            "-C",
            if self.optimize {
                "opt-level=3"
            } else {
                "opt-level=0"
            },
            "-C",
            &format!("target-cpu={cpu}"),
        ]
        .into_iter()
        .map(Into::into)
        .collect()
    }
}
/// Bumped when a required signed receipt field changes. Older receipts omit
/// `source` and are rejected before any native binary is allowed to run.
pub const RECEIPT_FORMAT: u32 = 2;

/// Signed attestation that build admission scanned and accepted the exact
/// installed package bytes whose native executable is being launched.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceAttestation {
    pub scanner_policy: String,
    pub archive_digest: String,
    pub tree_identity: String,
    pub source_kit: String,
}
impl SourceAttestation {
    pub(crate) fn is_well_formed(&self) -> bool {
        let sha256 = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        };
        self.scanner_policy == crate::source::SCAN_POLICY
            && sha256(&self.archive_digest)
            && sha256(&self.tree_identity)
            && sha256(&self.source_kit)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutableReceipt {
    pub format: u32,
    pub game: Receipt,
    pub plan: Plan,
    pub cpu: String,
    pub toolchain: String,
    pub arguments: Vec<String>,
    pub runtime: String,
    pub shader_preparation: Vec<String>,
    pub binary: String,
    pub source: SourceAttestation,
    #[serde(default)]
    pub bootstrap: Bootstrap,
    #[serde(default)]
    pub raw_objects: Vec<ObjectHash>,
    #[serde(default)]
    pub object_normalization: ObjectNormalization,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Bootstrap {
    #[default]
    HistoricalPipe,
    InProcess,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectNormalization {
    #[default]
    None,
    RustMingwSymbolExclusionsV1,
}
pub struct Artifact {
    manifest: crate::manifest::Manifest,
    directory: tempfile::TempDir,
    receipt: ExecutableReceipt,
}
impl Artifact {
    #[cfg(all(
        feature = "gui",
        any(windows, all(target_os = "linux", target_arch = "x86_64"))
    ))]
    pub(crate) fn from_verified_cache(
        manifest: crate::manifest::Manifest,
        receipt: ExecutableReceipt,
        bytes: &[u8],
    ) -> Result<Self> {
        let directory = tempfile::tempdir()?;
        fs::write(directory.path().join(filename(receipt.plan.target)), bytes)?;
        let artifact = Self {
            manifest,
            directory,
            receipt,
        };
        artifact.executable()?;
        Ok(artifact)
    }
    pub fn manifest(&self) -> &crate::manifest::Manifest {
        &self.manifest
    }
    pub fn receipt(&self) -> &ExecutableReceipt {
        &self.receipt
    }
    pub fn executable(&self) -> Result<PathBuf> {
        ensure!(
            self.receipt.format == RECEIPT_FORMAT
                && self.receipt.source.is_well_formed()
                && self.receipt.game.sdk == super::build_identity()
                && self.receipt.game.policy == crate::build_policy::REVISION,
            "stale service SDK/admission receipt"
        );
        ensure!(
            !self.receipt.raw_objects.is_empty()
                && self.receipt.raw_objects.len() == self.receipt.game.objects.len(),
            "incomplete compiler-object provenance"
        );
        for (raw, audited) in self
            .receipt
            .raw_objects
            .iter()
            .zip(&self.receipt.game.objects)
        {
            ensure!(
                raw.name == audited.name
                    && raw.sha256.len() == 64
                    && raw
                        .sha256
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "invalid compiler-object provenance"
            );
        }
        match self.receipt.object_normalization {
            ObjectNormalization::None => ensure!(
                self.receipt.raw_objects == self.receipt.game.objects,
                "unrecorded object transformation"
            ),
            ObjectNormalization::RustMingwSymbolExclusionsV1 => ensure!(
                self.receipt.plan.target.windows(),
                "COFF normalization on a non-Windows target"
            ),
        }
        let path = self
            .directory
            .path()
            .join(if self.receipt.plan.target.windows() {
                "game.exe"
            } else {
                "game"
            });
        let bytes = crate::files::read_regular(&path, 256 * 1024 * 1024, false)?;
        ensure!(
            crate::verify::digest(&bytes) == self.receipt.binary,
            "service executable changed after linking"
        );
        self.receipt.plan.target.validate(&bytes)?;
        Ok(path)
    }
    /// Host cache publication. A receipt is provenance, not a creator-controlled
    /// admission assertion. Profile policy must still be checked when loading.
    pub fn publish(&self, cache: &Path) -> Result<PathBuf> {
        let source = self.executable()?;
        fs::create_dir_all(cache)?;
        ensure!(
            fs::symlink_metadata(cache)?.is_dir(),
            "service cache must be a real directory"
        );
        let target = cache.join(if self.receipt.plan.target.windows() {
            "game.exe"
        } else {
            "game"
        });
        let mut file = tempfile::NamedTempFile::new_in(cache)?;
        use std::io::Write;
        file.write_all(&crate::files::read_regular(
            &source,
            256 * 1024 * 1024,
            false,
        )?)?;
        file.as_file().sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.as_file()
                .set_permissions(fs::Permissions::from_mode(0o500))?;
        }
        file.persist(&target).map_err(|e| e.error)?;
        let mut receipt = tempfile::NamedTempFile::new_in(cache)?;
        receipt.write_all(&serde_json::to_vec_pretty(&self.receipt)?)?;
        receipt.as_file().sync_all()?;
        receipt
            .persist(cache.join("services.json"))
            .map_err(|e| e.error)?;
        crate::integrity::record_build(
            &target,
            &crate::files::read_regular(&target, 256 * 1024 * 1024, false)?,
        )?;
        Ok(target)
    }
}

struct Tools {
    root: PathBuf,
    rustc: PathBuf,
    nm: PathBuf,
    linker: PathBuf,
    identity: String,
}
fn hash(path: &Path) -> Result<String> {
    use std::io::Read;
    let limit = crate::compiler::MAX_TOOL_BYTES;
    let file = crate::files::open_regular(&path.canonicalize()?, limit, false)?;
    let mut bounded = file.take(limit + 1);
    let digest = crate::verify::digest_reader(&mut bounded)?;
    ensure!(bounded.limit() != 0, "compiler tool grew beyond 2 GiB");
    Ok(digest)
}
fn tools(cache: &Path, target: Target) -> Result<Tools> {
    #[cfg(target_os = "linux")]
    let root = {
        let output = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()?;
        ensure!(output.status.success(), "Rust toolchain unavailable");
        PathBuf::from(std::str::from_utf8(&output.stdout)?.trim()).canonicalize()?
    };
    #[cfg(windows)]
    let root = crate::windows_sdk::prepare_source_target(cache, target.triple())?
        .0
        .canonicalize()?;
    #[cfg(not(any(windows, target_os = "linux")))]
    anyhow::bail!("confined native preparation requires Linux or Windows");
    #[cfg(target_os = "linux")]
    let _ = cache;
    let rustc = root.join(if cfg!(windows) {
        "bin/rustc.exe"
    } else {
        "bin/rustc"
    });
    #[cfg(target_os = "linux")]
    let (nm, linker) = (
        PathBuf::from("/usr/bin/llvm-nm-18"),
        PathBuf::from(match target {
            Target::Windows64 => "/usr/bin/x86_64-w64-mingw32-gcc",
            Target::Windows32 => "/usr/bin/i686-w64-mingw32-gcc",
            // Use the host's selected GCC. Its canonical target is hashed
            // below, so an alternatives change cannot swap an unaudited tool.
            _ => "/usr/bin/gcc",
        }),
    );
    #[cfg(windows)]
    let (nm, linker, auditor_identity) = {
        let host = if cfg!(target_arch = "x86") {
            "i686-pc-windows-gnu"
        } else {
            "x86_64-pc-windows-gnu"
        };
        let (nm, identity) = crate::windows_sdk::prepare_auditor(cache, host)?;
        (
            nm,
            root.join(format!(
                "lib/rustlib/{}/bin/self-contained/ld.exe",
                target.triple()
            )),
            identity,
        )
    };
    let rustc = rustc.canonicalize()?;
    let nm = nm.canonicalize()?;
    let linker = linker.canonicalize()?;
    let mut records = vec![hash(&rustc)?, hash(&nm)?, hash(&linker)?];
    #[cfg(target_os = "linux")]
    {
        // The GCC driver itself does not identify collect2, the real linker or
        // target CRT/import libraries. Bind the selected native tool closure.
        for (kind, names) in [
            ("prog", vec!["collect2", "ld"]),
            (
                "file",
                vec![
                    "crt1.o",
                    "Scrt1.o",
                    "crt2.o",
                    "crti.o",
                    "crtn.o",
                    "crtbegin.o",
                    "crtbeginS.o",
                    "crtend.o",
                    "crtendS.o",
                    "libgcc.a",
                    "libgcc_eh.a",
                    "libgcc_s.so",
                    "libc.so",
                    "libmingw32.a",
                    "libmingwex.a",
                    "libmsvcrt.a",
                    "libkernel32.a",
                    "liblto_plugin.so",
                ],
            ),
        ] {
            for name in names {
                let mut query = Command::new(&linker);
                query
                    .env_clear()
                    .env("PATH", "/usr/bin:/bin")
                    .stdin(std::process::Stdio::null());
                if target == Target::Linux32 {
                    query.arg("-m32");
                }
                query.arg(format!("-print-{kind}-name={name}"));
                let result =
                    crate::compiler::run_timed(&mut query, std::time::Duration::from_secs(5))?;
                ensure!(
                    result.status.success(),
                    "cannot identify native linker inputs"
                );
                let text = result.stdout.trim();
                let path = if Path::new(text).is_absolute() {
                    PathBuf::from(text)
                } else {
                    PathBuf::from("/usr/bin").join(text)
                };
                if path.is_file() {
                    records.push(format!("{kind}:{name}:{}", hash(&path.canonicalize()?)?));
                } else {
                    ensure!(
                        kind != "prog",
                        "native linker component unavailable: {name}"
                    );
                    records.push(format!("absent:{name}"));
                }
            }
        }
    }
    #[cfg(windows)]
    records.push(auditor_identity);
    let mut libraries = fs::read_dir(root.join("lib/rustlib").join(target.triple()).join("lib"))
        .context("install the selected Rust target library")?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    libraries.sort();
    ensure!(
        libraries.len() <= 512,
        "unexpected target library inventory"
    );
    for path in libraries {
        if path.extension().is_some_and(|s| s == "rlib") {
            records.push(format!(
                "{}:{}",
                path.file_name().unwrap().to_string_lossy(),
                hash(&path)?
            ));
        }
    }
    for entry in fs::read_dir(root.join("lib"))?.chain(fs::read_dir(root.join("bin"))?) {
        let path = entry?.path();
        let name = path.file_name().unwrap().to_string_lossy();
        if name.contains("rustc_driver") || name.starts_with("libLLVM") || name.starts_with("LLVM")
        {
            records.push(format!("{name}:{}", hash(&path)?));
        }
    }
    records.sort();
    let identity = crate::verify::digest(&serde_json::to_vec(&records)?);
    Ok(Tools {
        root,
        rustc,
        nm,
        linker,
        identity,
    })
}

fn run(
    tools: &Tools,
    source: &Path,
    output: &Path,
    audited: Option<&Path>,
    plan: &Plan,
    args: &[OsString],
) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let jobs = super::compiler_jobs::Jobs::new()?;
        let mut command = Command::new("prlimit");
        command.stdin(std::process::Stdio::null());
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command
            .arg(format!("--as={}", plan.compiler_memory_bytes))
            .arg(format!("--fsize={MAX_WORKER_FILE_BYTES}"));
        command.args([
            "--core=0",
            "--",
            "timeout",
            &COMPILER_TIMEOUT_SECS.to_string(),
            "bwrap",
            "--unshare-all",
            "--die-with-parent",
            "--new-session",
            "--clearenv",
            "--ro-bind",
            "/usr",
            "/usr",
        ]);
        for path in [
            "/bin",
            "/lib",
            "/lib64",
            "/lib32",
            "/libx32",
            "/etc/ld.so.cache",
        ] {
            if Path::new(path).exists() {
                command.args(["--ro-bind", path, path]);
            }
        }
        command
            .arg("--ro-bind")
            .arg(&tools.root)
            .arg("/rust")
            .arg("--ro-bind")
            .arg(source)
            .arg("/source")
            .arg("--bind")
            .arg(output)
            .arg("/output");
        if let Some(audited) = audited {
            command.arg("--ro-bind").arg(audited).arg("/audited");
        }
        jobs.configure(&mut command);
        command.args([
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--setenv",
            "PATH",
            "/rust/bin:/usr/bin:/bin",
            "--setenv",
            "LANG",
            "C.UTF-8",
            "--chdir",
            "/source",
            "--",
            "/rust/bin/rustc",
        ]);
        command.args(args);
        let result = crate::compiler::run(&mut command)?;
        jobs.check_diagnostics(&result.stderr)?;
        ensure!(
            result.status.success(),
            "confined service compilation failed ({}): {}",
            result.status,
            result.stderr
        );
    }
    #[cfg(windows)]
    {
        use gamepkg_native_windows_runner::Sandbox;
        let identity = crate::verify::digest(source.as_os_str().as_encoded_bytes());
        let sandbox = Sandbox::open(&format!("GamePKG.ServiceCompiler.{}", &identity[..32]))?;
        sandbox.grant_runtime(&tools.root)?;
        sandbox.grant_runtime(source)?;
        sandbox.grant_saves(output)?;
        if let Some(audited) = audited {
            sandbox.grant_runtime(audited)?;
        }
        let env = vec![
            (
                "SystemRoot".into(),
                std::env::var_os("SystemRoot").context("Windows system directory unavailable")?,
            ),
            (
                "LOCALAPPDATA".into(),
                std::env::var_os("LOCALAPPDATA").context("Windows profile unavailable")?,
            ),
            (
                "PATH".into(),
                std::env::join_paths([
                    tools.root.join("bin"),
                    tools.linker.parent().unwrap().to_path_buf(),
                ])?,
            ),
            ("TEMP".into(), output.as_os_str().to_owned()),
            ("TMP".into(), output.as_os_str().to_owned()),
        ];
        crate::windows_compiler::run_with_memory(
            &tools.rustc,
            args,
            source,
            &env,
            &sandbox,
            std::time::Instant::now() + std::time::Duration::from_secs(COMPILER_TIMEOUT_SECS),
            plan.compiler_memory_bytes,
        )?;
    }
    Ok(())
}
fn compiler_path(host: &Path, inside: &str, name: &str) -> PathBuf {
    if cfg!(target_os = "linux") {
        Path::new(inside).join(name)
    } else {
        host.join(name)
    }
}

fn compiler_objects(directory: &Path, units: u32) -> Result<Vec<PathBuf>> {
    ensure!(
        (1..=MAX_CODEGEN_UNITS).contains(&units),
        "invalid compiler unit bound"
    );
    let mut objects = Vec::new();
    let mut aggregate = None;
    let mut remaining = MAX_WORKER_FILE_BYTES;
    let mut indices = std::collections::BTreeSet::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        ensure!(
            entry.file_type()?.is_file(),
            "compiler output is not an individual regular file"
        );
        let size = entry.metadata()?.len();
        ensure!(
            size <= MAX_WORKER_FILE_BYTES,
            "compiler output exceeds the per-file cap"
        );
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-UTF-8 compiler output"))?;
        if name != "game_unit.o" {
            ensure!(
                size <= remaining,
                "compiler object set exceeds the total cap"
            );
            remaining -= size;
        }
        if units == 1 {
            ensure!(
                name == "game.o",
                "unexpected single-unit compiler output: {name}"
            );
            objects.push(entry.path());
            continue;
        }
        if name == "game_unit.o" {
            aggregate = Some(entry.path());
            continue;
        }
        ensure!(
            name.starts_with("game_unit.") && crate::archive::valid_path(&name),
            "unexpected compiler object name: {name}"
        );
        let stem = name
            .strip_suffix(".rcgu.o")
            .context("compiler output is not a retained codegen object")?;
        let (_, index) = stem
            .rsplit_once("-cgu.")
            .context("compiler object has no unit index")?;
        let index = index
            .parse::<u32>()
            .context("invalid compiler unit index")?;
        ensure!(
            index < units && indices.insert(index),
            "duplicate or out-of-range compiler unit index"
        );
        objects.push(entry.path());
    }
    objects.sort();
    ensure!(
        !objects.is_empty() && objects.len() <= units as usize,
        "incomplete compiler object inventory"
    );
    if let Some(aggregate) = aggregate {
        // Rustc copies its sole retained object to the requested crate name
        // when all partitions merge. Never link both copies or silently discard
        // a different aggregate object produced alongside multiple units.
        ensure!(
            objects.len() == 1,
            "unexpected aggregate with multiple compiler units"
        );
        let first = super::admission::hash_game_object(&objects[0])?;
        let copied = super::admission::hash_game_object(&aggregate)?;
        ensure!(
            first == copied,
            "aggregate differs from the retained compiler unit"
        );
    }
    Ok(objects)
}

struct CompiledGame {
    audited: AuditedStage,
    raw_objects: Vec<ObjectHash>,
    object_normalization: ObjectNormalization,
}

// Both bootstraps must enter the link phase through the same source and
// object checks. Compiler staging directories are dropped before linking.
fn compile_and_audit(
    tools: &Tools,
    sources: &CheckedSources,
    plan: &Plan,
    shaders: Vec<String>,
) -> Result<CompiledGame> {
    let input = tempfile::tempdir()?;
    let output = tempfile::tempdir()?;
    for (name, bytes) in sources.files() {
        if matches!(name, "source/Cargo.toml" | "source/Cargo.lock") {
            continue;
        }
        let relative = name
            .strip_prefix("source/src/")
            .filter(|name| name.ends_with(".rs"))
            .context("only complete checked Rust game units may enter service preparation")?;
        ensure!(
            crate::archive::valid_path(relative),
            "invalid checked source path"
        );
        let path = input.path().join(relative);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, bytes)?;
    }
    let root = input.path().join("lib.rs");
    let mut text = fs::read_to_string(&root)?;
    let client = super::CLIENT_SOURCE;
    let start = client
        .find("/// Safe dependency-free generated Rust binding.")
        .context("SDK frontend marker missing")?;
    let end = client
        .find("#[cfg(all(test, unix))]")
        .context("SDK test marker missing")?;
    text.push_str("\n#[allow(dead_code)] mod gamepkg {const MAX:usize=16*1024*1024;unsafe extern \"C\" {fn gamepkg_port_call(operation:u32,request:*const u8,length:u32,reply:*mut u8,capacity:u32)->i32;}\n");
    let frontend = &client[start..end];
    if plan.entry == "game_tick" {
        text.push_str(
            &frontend
                .replace("struct InputEvent", "struct NativeInputEvent")
                .replace("Vec<InputEvent>", "Vec<NativeInputEvent>")
                .replace("map(|b| InputEvent", "map(|b| NativeInputEvent")
                .replace("fn input_events(", "fn native_input_events(")
                .replace("fn file_read(", "fn scoped_file_read(")
                .replace("fn file_close(", "fn scoped_file_close("),
        );
    } else {
        text.push_str(frontend);
    }
    text.push_str(include_str!("../../engines/native-services/game_state.rs"));
    if plan.entry == "game_tick" {
        text.push_str(include_str!("../../engines/native-services/tick2d.rs"));
        let helpers = include_str!("../guest_helpers.rs.txt");
        let audio = helpers
            .find("/// A signed Slang")
            .context("guest audio marker missing")?;
        let files = helpers
            .find("/// One player-selected file")
            .context("guest file marker missing")?;
        text.push_str(&helpers[..audio]);
        text.push_str(&helpers[files..]);
    }
    text.push_str("\n}\n");
    if plan.entry == "game_tick" {
        text.push_str(
            r#"
#[export_name="game_service_entry"]
pub extern "C" fn __gamepkg_entry() {
    let mut state = 0;
    let mut previous_pad = 0;
    loop {
        match gamepkg::__gamepkg_tick_window() {
            Ok(Some(_)) => {}
            Ok(None) | Err(-4) => break,
            Err(_) => panic!("GamePKG tick window request failed"),
        }
        state = game_tick(gamepkg::__gamepkg_tick_input(&mut previous_pad), state);
        match gamepkg::__gamepkg_tick_present().and_then(|()| gamepkg::__gamepkg_tick_wait()) {
            Ok(()) => {}
            Err(-4) => break,
            Err(_) => panic!("GamePKG tick presentation or pacing failed"),
        }
    }
}
"#,
        );
    } else {
        text.push_str(&format!(
            "#[export_name=\"game_service_entry\"] pub extern \"C\" fn __gamepkg_entry(){{{}();}}\n",
            plan.entry
        ));
    }
    for (index, name) in plan.tasks.iter().enumerate() {
        text.push_str(&format!("#[export_name=\"game_task_{index:03}\"] pub extern \"C\" fn __gamepkg_task_{index}(arg:u64)->u64{{{name}(arg)}}\n"));
    }
    fs::write(&root, text)?;
    let mut args = plan.arguments();
    args.extend([
        "--crate-name".into(),
        "game_unit".into(),
        "--crate-type=lib".into(),
        "--emit=obj".into(),
        compiler_path(input.path(), "/source", "lib.rs").into_os_string(),
    ]);
    if plan.codegen_units == 1 {
        args.extend([
            "-o".into(),
            compiler_path(output.path(), "/output", "game.o").into_os_string(),
        ]);
    } else {
        // Rustc may merge small partitions. Its retained rcgu objects are the
        // individual optimizer outputs, not a prelinked archive or executable.
        args.extend([
            "--out-dir".into(),
            compiler_path(output.path(), "/output", "").into_os_string(),
        ]);
    }
    run(tools, input.path(), output.path(), None, plan, &args)?;
    let game_objects = compiler_objects(output.path(), plan.codegen_units)?;
    let mut raw_objects = Vec::new();
    let mut object_normalization = ObjectNormalization::None;
    for (index, object) in game_objects.iter().enumerate() {
        let sha256 = if plan.target.windows() {
            // COFF stores section and symbol offsets in 32-bit fields.
            let limit = super::admission::MAX_GAME_OBJECT_BYTES.min(u64::from(u32::MAX));
            let bytes = crate::files::read_regular(object, limit, false)?;
            let hash = crate::verify::digest(&bytes);
            if let Some(normalized) = super::coff_exclusions::normalize(&bytes)? {
                fs::write(object, normalized)?;
                object_normalization = ObjectNormalization::RustMingwSymbolExclusionsV1;
            }
            hash
        } else {
            super::admission::hash_game_object(object)?
        };
        raw_objects.push(ObjectHash {
            name: format!("game-{index:04}.o"),
            sha256,
        });
    }
    // The game compiler has exited. Seal copies outside all writable compiler
    // mounts; only the exact audited copies are provided to the trusted linker.
    let audited = AuditedStage::seal(
        sources,
        &tools.nm,
        &tools.rustc,
        plan.target.triple(),
        shaders,
        &game_objects,
    )?;
    Ok(CompiledGame {
        audited,
        raw_objects,
        object_normalization,
    })
}

/// Builds fixtures for the historical private-transport tests. The production
/// native bootstrap must link the in-process compatibility implementation.
pub fn build(
    cache: &Path,
    sources: &CheckedSources,
    plan: &Plan,
    shaders: Vec<String>,
    shader_preparation: Vec<String>,
) -> Result<Artifact> {
    ensure!(
        shader_preparation.len() <= 512
            && shader_preparation.iter().all(|s| s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())),
        "invalid shader preparation identity"
    );
    let tools = tools(cache, plan.target)?;
    let game = compile_and_audit(&tools, sources, plan, shaders)?;
    let staged = &game.audited;
    let objects = staged.objects()?;
    let audited = objects[0].parent().unwrap();
    let runtime = tempfile::tempdir()?;
    let linked = tempfile::tempdir()?;
    for (name, bytes) in [
        ("client.rs", super::CLIENT_SOURCE),
        (
            "local_bindings.rs",
            include_str!("../../engines/native-services/local_bindings.rs"),
        ),
        (
            "local_runtime.rs",
            include_str!("../../engines/native-services/local_runtime.rs"),
        ),
        (
            "local_memory.rs",
            include_str!("../../engines/native-services/local_memory.rs"),
        ),
        (
            "performance_clock.rs",
            include_str!("../../engines/native-services/performance_clock.rs"),
        ),
        (
            "local_timing.rs",
            include_str!("../../engines/native-services/local_timing.rs"),
        ),
        (
            "local_timing_windows.rs",
            include_str!("../../engines/native-services/local_timing_windows.rs"),
        ),
        (
            "lockdown.rs",
            include_str!("../../engines/native-services/lockdown.rs"),
        ),
    ] {
        fs::write(runtime.path().join(name), bytes)?;
    }
    let (windows, tasks, table) = runtime_bindings(plan);
    let bootstrap = format!(
        r##"#![allow(dead_code)]
#[path="client.rs"]mod client;#[path="local_bindings.rs"]mod local;
#[path="lockdown.rs"]mod lockdown;
unsafe extern "C"{{fn game_service_entry();}}
{tasks}
fn main(){{
 #[cfg(unix)]let (input,output)=unsafe{{use std::os::fd::FromRawFd;(std::fs::File::from_raw_fd(0),std::fs::File::from_raw_fd(1))}};
 #[cfg(windows)]let (input,output)=unsafe{{use std::os::windows::io::{{AsRawHandle,FromRawHandle}};(std::fs::File::from_raw_handle(std::io::stdin().as_raw_handle()),std::fs::File::from_raw_handle(std::io::stdout().as_raw_handle()))}};
 client::connect(input,output).expect("private transport");
 local::install(local::runtime::Runtime::new({windows},vec![{table}],1024*1024*1024,32,1024*1024).expect("fixed local runtime")).expect("local binding");
 lockdown::install().expect("process confinement");
 unsafe{{game_service_entry();}}local::shutdown();client::disconnect();
}}
"##,
    );
    fs::write(runtime.path().join("main.rs"), &bootstrap)?;
    let runtime_hash =
        crate::verify::digest(format!("{bootstrap}\0{}", super::build_identity()).as_bytes());
    let filename = filename(plan.target);
    let mut link = plan.arguments();
    link.extend([
        compiler_path(runtime.path(), "/source", "main.rs").into_os_string(),
        "-o".into(),
        compiler_path(linked.path(), "/output", filename).into_os_string(),
        "-C".into(),
        format!("linker={}", tools.linker.display()).into(),
    ]);
    for object in &objects {
        link.extend([
            "-C".into(),
            format!(
                "link-arg={}",
                compiler_path(
                    audited,
                    "/audited",
                    object.file_name().unwrap().to_str().unwrap()
                )
                .display()
            )
            .into(),
        ]);
    }
    staged.verify()?;
    run(
        &tools,
        runtime.path(),
        linked.path(),
        Some(audited),
        plan,
        &link,
    )?;
    finish(
        sources,
        plan,
        game,
        LinkedRuntime {
            directory: linked,
            bootstrap: Bootstrap::HistoricalPipe,
            toolchain: tools.identity,
            identity: runtime_hash,
            shader_preparation,
        },
    )
}

fn runtime_bindings(plan: &Plan) -> (String, String, String) {
    let windows = plan
        .layout
        .windows()
        .iter()
        .map(|window| {
            format!(
                "local::runtime::Window{{address:{:#x},bytes:{:#x}}}",
                window.address, window.bytes
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let windows = format!("&[{windows}]");
    let mut tasks = String::new();
    let mut table = Vec::new();
    for index in 0..plan.tasks.len() {
        tasks.push_str(&format!("unsafe extern \"C\" {{fn game_task_{index:03}(arg:u64)->u64;}} fn dispatch_{index}(_:local::runtime::TaskContext,arg:u64)->u64{{unsafe{{game_task_{index:03}(arg)}}}}\n"));
        table.push(format!("dispatch_{index}"));
    }
    (windows, tasks, table.join(","))
}

fn filename(target: Target) -> &'static str {
    if target.windows() {
        "game.exe"
    } else {
        "game"
    }
}

struct LinkedRuntime {
    directory: tempfile::TempDir,
    bootstrap: Bootstrap,
    toolchain: String,
    identity: String,
    shader_preparation: Vec<String>,
}

fn finish(
    sources: &CheckedSources,
    plan: &Plan,
    game: CompiledGame,
    linked: LinkedRuntime,
) -> Result<Artifact> {
    let CompiledGame {
        audited,
        raw_objects,
        object_normalization,
    } = game;
    let LinkedRuntime {
        directory,
        bootstrap,
        toolchain,
        identity,
        shader_preparation,
    } = linked;
    audited.verify()?;
    let filename = filename(plan.target);
    let binary =
        crate::files::read_regular(&directory.path().join(filename), 256 * 1024 * 1024, false)?;
    plan.target.validate(&binary)?;
    let receipt = ExecutableReceipt {
        format: RECEIPT_FORMAT,
        game: audited.receipt().clone(),
        plan: plan.clone(),
        cpu: if plan.target.native() && plan.optimize {
            crate::native::cpu_identity().into()
        } else {
            "portable".into()
        },
        toolchain,
        arguments: plan
            .arguments()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect(),
        runtime: identity,
        shader_preparation,
        binary: crate::verify::digest(&binary),
        source: SourceAttestation {
            scanner_policy: crate::source::SCAN_POLICY.into(),
            archive_digest: sources.archive_digest().into(),
            tree_identity: sources.identity().into(),
            source_kit: sources.identity().into(),
        },
        bootstrap,
        raw_objects,
        object_normalization,
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            directory.path().join(filename),
            fs::Permissions::from_mode(0o500),
        )?;
    }
    let artifact = Artifact {
        manifest: sources.manifest().clone(),
        directory,
        receipt,
    };
    artifact.executable()?;
    Ok(artifact)
}

#[cfg(test)]
mod compiler_inventory_tests {
    use super::*;
    #[test]
    fn codegen_inventory_requires_all_regular_unique_bounded_objects() {
        let root = tempfile::tempdir().unwrap();
        assert!(compiler_objects(root.path(), 4).is_err());
        let first = root.path().join("game_unit.game_unit.abc-cgu.0.rcgu.o");
        fs::write(&first, b"first").unwrap();
        assert_eq!(
            compiler_objects(root.path(), 4).unwrap().as_slice(),
            std::slice::from_ref(&first)
        );
        let aggregate = root.path().join("game_unit.o");
        fs::write(&aggregate, b"different").unwrap();
        assert!(compiler_objects(root.path(), 4).is_err());
        fs::write(&aggregate, b"first").unwrap();
        assert_eq!(
            compiler_objects(root.path(), 4).unwrap().as_slice(),
            std::slice::from_ref(&first)
        );
        let second = root.path().join("game_unit.game_unit.abc-cgu.1.rcgu.o");
        fs::write(&second, b"second").unwrap();
        assert!(compiler_objects(root.path(), 4).is_err());
        fs::remove_file(aggregate).unwrap();
        assert_eq!(compiler_objects(root.path(), 4).unwrap(), [first, second]);
        for name in [
            "foreign.o",
            "game_unit.rmeta",
            "game_unit.other-cgu.0.rcgu.o",
            "game_unit.other-cgu.4.rcgu.o",
        ] {
            let bad = root.path().join(name);
            fs::write(&bad, b"bad").unwrap();
            assert!(compiler_objects(root.path(), 4).is_err(), "{name}");
            fs::remove_file(bad).unwrap();
        }
        fs::create_dir(root.path().join("subdirectory")).unwrap();
        assert!(compiler_objects(root.path(), 4).is_err());
    }
    #[test]
    fn legacy_single_object_has_no_unreviewed_extra_outputs() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("game.o"), b"single").unwrap();
        assert_eq!(
            compiler_objects(root.path(), 1).unwrap(),
            [root.path().join("game.o")]
        );
        fs::write(root.path().join("extra.o"), b"extra").unwrap();
        assert!(compiler_objects(root.path(), 1).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn codegen_outputs_cannot_be_symlinks_or_directories() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::os::unix::fs::symlink(
            outside.path(),
            root.path().join("game_unit.game_unit.abc-cgu.0.rcgu.o"),
        )
        .unwrap();
        assert!(compiler_objects(root.path(), 4).is_err());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod compiler_output_tests {
    use super::*;

    #[test]
    fn worker_output_inventory_checks_each_file_and_combined_size() {
        let output = tempfile::tempdir().unwrap();
        let first = output.path().join("game_unit.fixture-cgu.0.rcgu.o");
        let first = fs::File::create(first).unwrap();
        first.set_len(MAX_WORKER_FILE_BYTES + 1).unwrap();
        let inventory = || compiler_objects(output.path(), DEFAULT_CODEGEN_UNITS);
        assert!(inventory()
            .unwrap_err()
            .to_string()
            .contains("per-file cap"));
        first.set_len(MAX_WORKER_FILE_BYTES).unwrap();
        assert_eq!(inventory().unwrap().len(), 1);

        fs::write(output.path().join("game_unit.fixture-cgu.1.rcgu.o"), b"o").unwrap();
        assert!(inventory().unwrap_err().to_string().contains("total cap"));
    }
}
