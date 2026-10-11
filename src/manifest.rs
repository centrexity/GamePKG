use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format_version: u32,
    pub id: String,
    pub version: String,
    pub entry_world: String,
    pub lockfile_sha256: String,
    pub limits: Limits,
    pub permissions: Permissions,
    pub shader_profile: String,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub memory_bytes: u64,
    pub fuel: u64,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    pub input: bool,
    pub audio: bool,
    pub save: bool,
    pub network: String,
}
impl Manifest {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let m: Self = toml::from_str(std::str::from_utf8(bytes)?)?;
        if m.format_version != 1 {
            bail!("unsupported format version");
        }
        if m.id.is_empty()
            || !m
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b".-_".contains(&c))
        {
            bail!("invalid package id");
        }
        if m.version.is_empty() || m.entry_world.is_empty() {
            bail!("version and entry world are required");
        }
        if m.limits.memory_bytes == 0
            || m.limits.memory_bytes > 512 * 1024 * 1024
            || m.limits.fuel == 0
        {
            bail!("invalid resource limits");
        }
        if m.permissions.network != "none" {
            bail!("network capabilities are not implemented");
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
