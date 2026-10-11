use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format_version: u32,
    #[serde(default = "default_engine_version")]
    pub engine_version: String,
    #[serde(default = "default_save_schema")]
    pub save_schema: u32,
    pub id: String,
    pub version: String,
    pub entry_world: String,
    #[serde(default = "default_runtime_profile")]
    pub runtime_profile: String,
    pub lockfile_sha256: String,
    pub limits: Limits,
    pub permissions: Permissions,
    pub shader_profile: String,
    #[serde(default)]
    pub shaders: Vec<Shader>,
    #[serde(default)]
    pub creator_key: String,
    #[serde(default)]
    pub update_url: Option<String>,
    #[serde(default)]
    pub requires_rom: bool,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Shader {
    pub source: String,
    pub entry: String,
    pub stage: String,
}
impl Shader {
    pub fn validate(&self) -> Result<()> {
        if !crate::archive::valid_path(&self.source)
            || !self.source.starts_with("shaders/")
            || !self.source.ends_with(".slang")
        {
            bail!("invalid shader source path");
        }
        if self.entry.is_empty()
            || self.entry.len() > 128
            || !self
                .entry
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || self.entry.as_bytes()[0].is_ascii_digit()
        {
            bail!("invalid shader entry point");
        }
        if !matches!(self.stage.as_str(), "vertex" | "fragment" | "compute") {
            bail!("unsupported shader stage");
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(alias = "memory_bytes")]
    pub hard_memory_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_memory_bytes: Option<u64>,
    pub fuel: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    pub input: bool,
    pub audio: bool,
    pub save: bool,
    pub network: String,
}
impl Manifest {
    pub fn ensure_engine_compatible(&self) -> Result<()> {
        if self.engine_version.trim().is_empty() || self.engine_version.len() > 256 {
            bail!("invalid engine version requirement");
        }
        let requirement = semver::VersionReq::parse(&self.engine_version)?;
        let engine = semver::Version::parse(env!("CARGO_PKG_VERSION"))?;
        if !requirement.matches(&engine) {
            bail!(
                "game requires engine {}, but this host provides {}",
                self.engine_version,
                engine
            );
        }
        Ok(())
    }
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let mut m: Self = toml::from_str(std::str::from_utf8(bytes)?)?;
        if m.format_version != 1 {
            bail!("unsupported format version");
        }
        if m.id.is_empty()
            || m.id.len() > 200
            || !m
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
        {
            bail!("invalid package id");
        }
        if m.version.len() > 256 || m.entry_world.len() > 128 {
            bail!("version or entry world is too long");
        }
        semver::Version::parse(&m.version)?;
        m.ensure_engine_compatible()?;
        if !matches!(m.runtime_profile.as_str(), "core-v1" | "component-v1") {
            bail!("unsupported runtime profile");
        }
        if m.save_schema == 0 {
            bail!("save schema must be positive");
        }
        if !m.creator_key.is_empty() {
            m.creator_key = hex::encode(crate::verify::decode_key(&m.creator_key)?);
        }
        if let Some(url) = &m.update_url {
            crate::registry::validate_url(url)?;
        }
        if m.id == "." || m.id == ".." {
            bail!("invalid package id");
        }
        if m.version.is_empty() || m.entry_world.is_empty() {
            bail!("version and entry world are required");
        }
        if m.limits.hard_memory_bytes == 0
            || m.limits.hard_memory_bytes > 512 * 1024 * 1024
            || m.limits.fuel == 0
        {
            bail!("invalid resource limits");
        }
        if let Some(recommended) = m.limits.recommended_memory_bytes {
            if recommended == 0 || recommended > m.limits.hard_memory_bytes {
                bail!(
                    "recommended memory must be positive and no greater than the hard memory limit"
                );
            }
        }
        if m.permissions.network != "none" {
            bail!("network capabilities are not implemented");
        }
        if m.shaders.len() > 32 {
            bail!("too many shader entry points");
        }
        for shader in &m.shaders {
            shader.validate()?;
        }
        if m.shader_profile != "slang" {
            bail!("Slang shader profile required");
        }
        if hex::decode(&m.lockfile_sha256)?.len() != 32 {
            bail!("invalid lockfile digest");
        }
        Ok(m)
    }
}

impl Limits {
    pub fn recommended_memory_bytes(&self) -> u64 {
        self.recommended_memory_bytes
            .unwrap_or(self.hard_memory_bytes)
    }
}

fn default_engine_version() -> String {
    "^0.1.0".into()
}
fn default_save_schema() -> u32 {
    1
}

fn default_runtime_profile() -> String {
    "core-v1".into()
}
