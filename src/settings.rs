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
    pub library_directory: String,
    pub tor_proxy: String,
    pub memory_ceiling_mib: u64,
    pub fuel_ceiling: u64,
    pub tick_deadline_ms: u64,
    pub optimize_builds: bool,
    pub dark_mode: bool,
    pub slang_directory: String,
    pub shader_backend: String,
    pub mute_audio: bool,
    pub audio_volume: u8,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            format_version: 1,
            library_directory: String::new(),
            tor_proxy: "127.0.0.1:9050".into(),
            memory_ceiling_mib: 512,
            fuel_ceiling: 10000000,
            tick_deadline_ms: 100,
            optimize_builds: true,
            dark_mode: true,
            slang_directory: String::new(),
            shader_backend: "spirv".into(),
            mute_audio: false,
            audio_volume: 50,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.format_version != 1 {
            bail!("unsupported settings version");
        }
        if !self.library_directory.is_empty()
            && (!Path::new(&self.library_directory).is_absolute()
                || self.library_directory.chars().any(char::is_control))
        {
            bail!("library directory must be an absolute path or empty for the default");
        }
        if !self.slang_directory.is_empty() && !Path::new(&self.slang_directory).is_absolute() {
            bail!("Slang folder must be an absolute path");
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
        validate_proxy(&self.tor_proxy)?;
        if !(1..=512).contains(&self.memory_ceiling_mib) {
            bail!("memory ceiling must be 1–512 MiB");
        }
        if self.fuel_ceiling == 0 || self.fuel_ceiling > 100000000 {
            bail!("fuel ceiling must be 1–100000000");
        }
        if !(10..=100).contains(&self.tick_deadline_ms) || !self.tick_deadline_ms.is_multiple_of(10)
        {
            bail!("tick deadline must be 10–100 ms in steps of 10");
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
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
        .context("set GAMEPKG_CONFIG to a settings file")?;
    Ok(root.join("gamepkg/settings.toml"))
}
pub fn load() -> Result<Settings> {
    Settings::read(&path()?)
}
