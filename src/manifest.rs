use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format_version: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub achievements: Vec<Achievement>,
    #[serde(default = "default_engine_version")]
    pub engine_version: String,
    #[serde(default = "default_save_schema")]
    pub save_schema: u32,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<String>,
    pub version: String,
    pub entry_world: String,
    #[serde(default = "default_runtime_profile")]
    pub runtime_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin_engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_data: Option<GameDataRequirement>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roms: Vec<RomRequirement>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub shared_assets: std::collections::BTreeMap<String, crate::pool::Reference>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GameDataRequirement {
    pub title: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub user_selected_servers: bool,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RomRequirement {
    pub id: String,
    pub title: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Achievement {
    pub id: String,
    pub title: String,
    pub description: String,
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
    pub recommended_storage_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_memory_bytes: Option<u64>,
    // Accepted only for packages signed with the earlier schema; never enforced or emitted.
    #[serde(default, rename = "fuel", skip_serializing)]
    pub legacy_execution_budget: Option<u64>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    pub input: bool,
    pub audio: bool,
    pub save: bool,
    pub network: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network_hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network_listen: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub network_allow_local: bool,
}
impl Manifest {
    pub fn uses_selected_servers(&self) -> bool {
        self.builtin_engine.as_deref() == Some(crate::cube2::PROFILE)
            && self
                .game_data
                .as_ref()
                .is_some_and(|data| data.user_selected_servers)
    }
    pub fn display_title(&self) -> &str {
        self.title.as_deref().unwrap_or(&self.id)
    }
    pub fn required_rom_count(&self) -> usize {
        if self.roms.is_empty() {
            usize::from(self.requires_rom)
        } else {
            self.roms.len()
        }
    }
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
        for (label, value, max, multiline) in [
            ("title", m.title.as_deref(), 256, false),
            ("description", m.description.as_deref(), 4096, true),
            ("author", m.author.as_deref(), 128, false),
        ] {
            if let Some(value) = value {
                validate_metadata_text(value, max, multiline, label)?;
            }
        }
        if let Some(homepage) = &m.homepage {
            if homepage.len() > 2048 {
                bail!("metadata homepage is too long");
            }
            crate::registry::validate_url(homepage)?;
        }
        if m.genres.len() > 8 {
            bail!("at most eight metadata genres are allowed");
        }
        let mut genres = std::collections::HashSet::new();
        for genre in &m.genres {
            validate_metadata_text(genre, 32, false, "genre")?;
            if !genres.insert(genre.trim().to_lowercase()) {
                bail!("duplicate metadata genre");
            }
        }
        if m.roms.len() > 8 {
            bail!("at most eight ROMs may be required");
        }
        let mut rom_ids = std::collections::HashSet::new();
        for rom in &m.roms {
            if !crate::progress::valid_id(&rom.id)
                || !rom_ids.insert(&rom.id)
                || rom.title.trim().is_empty()
                || rom.title.len() > 128
                || rom.title.chars().any(char::is_control)
            {
                bail!("invalid or duplicate ROM requirement");
            }
        }
        if !m.roms.is_empty() {
            m.requires_rom = true;
        }
        if m.achievements.len() > 128 {
            bail!("at most 128 achievements may be declared");
        }
        let mut ids = std::collections::HashSet::new();
        for a in &m.achievements {
            if !crate::progress::valid_id(&a.id)
                || !ids.insert(&a.id)
                || a.title.trim().is_empty()
                || a.title.len() > 128
                || a.description.len() > 512
                || a.title
                    .chars()
                    .chain(a.description.chars())
                    .any(char::is_control)
            {
                bail!("invalid or duplicate achievement declaration");
            }
        }
        let cube2 = m.builtin_engine.as_deref() == Some(crate::cube2::PROFILE);
        if cube2 {
            if !m.shaders.is_empty() || !m.shared_assets.is_empty() {
                bail!("Cube 2 cannot substitute package shaders or media for original game data");
            }
            let data = m
                .game_data
                .as_ref()
                .context("Cube 2 requires an original game-data folder declaration")?;
            if data.title.trim().is_empty()
                || data.title.len() > 256
                || data.title.chars().any(char::is_control)
                || m.runtime_profile != "core-v1"
                || m.entry_world != "game"
                || m.required_rom_count() != 0
                || !m.permissions.save
                || !m.permissions.audio
                || !m.permissions.input
                || m.limits.hard_memory_bytes < crate::cube2::MEMORY
            {
                bail!("Cube 2 requires a data folder, input/save/audio permissions and at least 2 GiB memory");
            }
        } else {
            if m.game_data.is_some() {
                bail!("game-data folders are only supported by the pinned Cube 2 engine");
            }
            if let Some(engine) = &m.builtin_engine {
                let minimum = crate::builtin::memory(engine)? + 16 * 1048576;
                if !matches!(
                    engine.as_str(),
                    crate::zelda3::PROFILE | crate::doom::PROFILE
                ) || m.runtime_profile != "core-v1"
                    || m.required_rom_count() != 1
                    || !m.permissions.save
                    || !m.permissions.audio
                    || m.limits.hard_memory_bytes < minimum
                {
                    bail!("builtin games require a pinned core-v1 engine, one ROM, save/audio permissions and enough memory for the engine plus 16 MiB");
                }
            }
        }
        if m.format_version != 1 {
            bail!("unsupported format version");
        }
        if !crate::archive::valid_path(&m.id)
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
            || m.limits.hard_memory_bytes
                > if cube2 {
                    4 * 1024 * 1024 * 1024
                } else {
                    512 * 1024 * 1024
                }
        {
            bail!("invalid resource limits");
        }
        if m.limits.recommended_storage_bytes == Some(0) {
            bail!("recommended storage must be positive");
        }
        if let Some(recommended) = m.limits.recommended_memory_bytes {
            if recommended == 0 || recommended > m.limits.hard_memory_bytes {
                bail!(
                    "recommended memory must be positive and no greater than the hard memory limit"
                );
            }
        }
        if m.uses_selected_servers() {
            crate::network::validate_user_selected(&m.permissions)?;
        } else {
            crate::network::validate(&m.permissions)?;
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
        if m.shared_assets.len() > 10000 {
            bail!("too many shared assets");
        }
        let mut total = 0u64;
        let mut sizes = std::collections::BTreeMap::new();
        for (path, reference) in &m.shared_assets {
            if path.len() > 1024 || !crate::archive::valid_path(path) {
                bail!("invalid shared asset path");
            }
            reference.validate()?;
            if sizes
                .insert(&reference.sha256, reference.size)
                .is_some_and(|size| size != reference.size)
            {
                bail!("shared digest has conflicting sizes");
            }
            total = total
                .checked_add(reference.size)
                .ok_or_else(|| anyhow::anyhow!("shared asset size overflow"))?;
        }
        if total > crate::archive::MAX_TOTAL {
            bail!("shared assets exceed package size limit");
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

/// Metadata is bounded plain text, never markup or terminal control sequences.
pub(crate) fn validate_metadata_text(
    value: &str,
    max: usize,
    multiline: bool,
    label: &str,
) -> Result<()> {
    if value.trim().is_empty()
        || value.len() > max
        || value.chars().any(|c| {
            (c.is_control() && !(multiline && c == '\n'))
                || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
    {
        bail!("invalid metadata {label}");
    }
    Ok(())
}
