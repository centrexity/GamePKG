use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub format_version: u32,
    #[serde(default, rename = "approved_crates", skip_serializing)]
    pub legacy_approved_crates: Vec<String>,
    pub library_directory: String,
    pub tor_proxy: String,
    pub memory_ceiling_mib: u64,
    #[serde(default, rename = "fuel_ceiling", skip_serializing)]
    pub legacy_execution_ceiling: Option<u64>,
    pub tick_deadline_ms: u64,
    pub optimize_builds: bool,
    pub dark_mode: bool,
    pub ui_scale_percent: u16,
    pub slang_directory: String,
    pub wasi_sysroot_directory: String,
    pub dxc_directory: String,
    pub windows_compiler_distribution: String,
    pub windows_compiler_executable: String,
    pub shader_backend: String,
    pub renderer: String,
    pub mute_audio: bool,
    pub audio_volume: u8,
    pub automatic_update_checks: bool,
    pub update_interval_hours: u64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            format_version: 1,
            legacy_approved_crates: Vec::new(),
            library_directory: String::new(),
            tor_proxy: "127.0.0.1:9050".into(),
            memory_ceiling_mib: 512,
            legacy_execution_ceiling: None,
            tick_deadline_ms: 100,
            optimize_builds: true,
            dark_mode: true,
            ui_scale_percent: 100,
            slang_directory: String::new(),
            wasi_sysroot_directory: String::new(),
            dxc_directory: String::new(),
            windows_compiler_distribution: String::new(),
            windows_compiler_executable: String::new(),
            shader_backend: "wgsl".into(),
            renderer: "vulkan".into(),
            mute_audio: false,
            audio_volume: 50,
            automatic_update_checks: true,
            update_interval_hours: 24,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        let distribution = &self.windows_compiler_distribution;
        let executable = &self.windows_compiler_executable;
        if distribution.is_empty() != executable.is_empty() {
            bail!(
                "Windows compiler settings need both a WSL distribution and Linux executable path"
            );
        }
        if !distribution.is_empty()
            && (distribution.len() > 128
                || !distribution
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
                || distribution.starts_with('-'))
        {
            bail!("WSL distribution must be a name of at most 128 letters, digits, dots, underscores or hyphens");
        }
        if !executable.is_empty()
            && (executable.len() > 4096
                || !executable.starts_with('/')
                || executable.contains('\\')
                || executable.chars().any(char::is_control))
        {
            bail!("Windows compiler executable must be an absolute Linux path without control characters");
        }
        if !(80..=160).contains(&self.ui_scale_percent) {
            bail!("Interface size must be 80–160 percent");
        }
        if self.format_version != 1 {
            bail!("unsupported settings version");
        }
        if !self.library_directory.is_empty()
            && (!Path::new(&self.library_directory).is_absolute()
                || self.library_directory.chars().any(char::is_control))
        {
            bail!("library directory must be an absolute path or empty for the default");
        }
        if !self.slang_directory.is_empty()
            && (!Path::new(&self.slang_directory).is_absolute()
                || self.slang_directory.chars().any(char::is_control))
        {
            bail!("Slang folder must be an absolute path or empty");
        }
        if !self.wasi_sysroot_directory.is_empty()
            && (!Path::new(&self.wasi_sysroot_directory).is_absolute()
                || self.wasi_sysroot_directory.chars().any(char::is_control))
        {
            bail!("C/WASI tools folder must be an absolute path or empty");
        }
        if !self.dxc_directory.is_empty()
            && (!Path::new(&self.dxc_directory).is_absolute()
                || self.dxc_directory.chars().any(char::is_control))
        {
            bail!("DXC folder must be an absolute path or empty");
        }
        if !matches!(self.renderer.as_str(), "vulkan" | "opengl") {
            bail!("unsupported renderer");
        }
        if !matches!(
            self.shader_backend.as_str(),
            "spirv" | "wgsl" | "metal" | "dxil" | "glsl"
        ) {
            bail!("unsupported shader backend");
        }
        if self.audio_volume > 100 {
            bail!("audio volume must be 0–100");
        }
        if !(1..=168).contains(&self.update_interval_hours) {
            bail!("update interval must be 1–168 hours");
        }
        validate_proxy(&self.tor_proxy)?;
        if !(1..=4096).contains(&self.memory_ceiling_mib) {
            bail!("memory ceiling must be 1–4096 MiB");
        }
        if !(10..=1000).contains(&self.tick_deadline_ms)
            || !self.tick_deadline_ms.is_multiple_of(10)
        {
            bail!("tick deadline must be 10–1000 ms in steps of 10");
        }
        Ok(())
    }
    pub fn read(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut bytes = Vec::new();
        fs::File::open(path)?.take(65537).read_to_end(&mut bytes)?;
        if bytes.len() > 65536 {
            bail!("settings file is too large");
        }
        let settings: Self = toml::from_str(std::str::from_utf8(&bytes)?)?;
        settings.validate()?;
        Ok(settings)
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        temp.write_all(toml::to_string(self)?.as_bytes())?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|e| e.error)?;
        Ok(())
    }
}
pub fn validate_proxy(value: &str) -> Result<std::net::SocketAddr> {
    let address: std::net::SocketAddr = value
        .parse()
        .context("Tor proxy must be a loopback IP:port")?;
    if !address.ip().is_loopback() || address.port() == 0 {
        bail!("Tor proxy must be a loopback IP with a nonzero port");
    }
    Ok(address)
}
pub fn path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("GAMEPKG_CONFIG") {
        return Ok(PathBuf::from(path));
    }
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(|p| PathBuf::from(p).join(".config"))
        })
        .context("set GAMEPKG_CONFIG to a settings file")?;
    Ok(root.join("gamepkg/settings.toml"))
}
pub fn load() -> Result<Settings> {
    let mut settings = Settings::read(&path()?)?;
    if let Ok(renderer) = std::env::var("GAMEPKG_RENDERER") {
        settings.renderer = renderer;
        settings.validate()?;
    }
    Ok(settings)
}
