use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
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
    #[serde(default, rename = "entry_world", skip_serializing)]
    legacy_entry_world: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub native_services: bool,
    #[serde(default = "default_runtime_profile")]
    pub runtime_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub builtin_engine: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub native_tick: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_data: Option<GameDataRequirement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch_options: Option<crate::launch_options::LaunchOptions>,
    pub lockfile_sha256: String,
    pub limits: Limits,
    pub permissions: Permissions,
    pub shader_profile: String,
    #[serde(default)]
    pub shaders: Vec<Shader>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_graphics: Option<NativeGraphics>,
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
    /// Signed data layout. Native services allow one read-only folder layout.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layouts: Vec<DataLayout>,
}
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataLayout {
    /// `link` exposes directories beside a private data root. `beside` places one archive next to the executable.
    #[serde(default = "default_layout_placement")]
    pub placement: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directories: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
}
fn default_layout_placement() -> String {
    "link".into()
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
/// Host-reviewed shader selectors for the fixed `textured-v1` raster profile.
pub const TEXTURED_V1_SHADER_SOURCE: &str = "shaders/textured_v1.slang";
pub const TEXTURED_V1_VERTEX_ENTRY: &str = "vertexMain";
pub const TEXTURED_V1_FRAGMENT_ENTRY: &str = "fragmentMain";
/// Host-reviewed shader selectors for the fixed `textured-v2` raster profile.
pub const TEXTURED_V2_SHADER_SOURCE: &str = "shaders/textured_v2.slang";
pub const TEXTURED_V2_VERTEX_ENTRY: &str = "vertexMain";
pub const TEXTURED_V2_FRAGMENT_ENTRY: &str = "fragmentMain";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeGraphics {
    VertexColorV1,
    TexturedV1,
    TexturedV2,
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
/// Fixed, signed renderer allowances; omitted profiles retain legacy defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum RenderProfile {
    #[serde(rename = "large-images-v1")]
    LargeImagesV1,
}
impl RenderProfile {
    pub const fn frame_bytes(self) -> usize {
        32 * 1048576
    }
    pub const fn cache_bytes(self) -> usize {
        128 * 1048576
    }
    pub const fn cache_paths(self) -> usize {
        256
    }
    pub const fn web_asset_bytes(self) -> usize {
        128 * 1048576
    }
    pub const fn minimum_memory_bytes(self) -> u64 {
        256 * 1048576
    }
}

const MAX_NATIVE_SAVE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(alias = "memory_bytes")]
    pub hard_memory_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_storage_bytes: Option<u64>,
    /// Aggregate logical bytes in the native compatibility save directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hard_save_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_memory_bytes: Option<u64>,
    /// Host-owned recorded effects share, rather than extend, the hard limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects_memory_bytes: Option<u64>,
    /// Host-owned asynchronous music shares, rather than extends, the hard limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_memory_bytes: Option<u64>,
    /// Separately bounded host rendering resources, never an ambient game API.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_profile: Option<RenderProfile>,
    // Accepted only for packages signed with the earlier schema; never enforced or emitted.
    #[serde(default, rename = "fuel", skip_serializing)]
    pub legacy_execution_budget: Option<u64>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<UserFile>,
    pub input: bool,
    pub audio: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub microphone: bool,
    pub save: bool,
    #[serde(deserialize_with = "network_permission")]
    pub network: bool,
    #[serde(default, skip_serializing)]
    pub network_hosts: Vec<String>,
    #[serde(default, skip_serializing)]
    pub network_listen: Vec<String>,
    #[serde(default, skip_serializing)]
    pub network_allow_local: bool,
}
// Earlier signed manifests used transport names and endpoint lists. Read those
// names as an on/off permission; new manifests emit a boolean only.
fn network_permission<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Value {
        Enabled(bool),
        Legacy(String),
    }
    match Value::deserialize(deserializer)? {
        Value::Enabled(enabled) => Ok(enabled),
        Value::Legacy(value) => match value.as_str() {
            "none" => Ok(false),
            "https" | "multiplayer" | "all" => Ok(true),
            _ => Err(serde::de::Error::custom("network must be true or false")),
        },
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserFile {
    pub id: String,
    pub title: String,
    pub extensions: Vec<String>,
    pub max_bytes: u64,
    #[serde(default)]
    pub read: bool,
    #[serde(default)]
    pub write: bool,
}
impl Manifest {
    #[cfg(any(
        test,
        all(
            feature = "gui",
            any(windows, all(target_os = "linux", target_arch = "x86_64"))
        )
    ))]
    pub(crate) fn native_save_quota(&self) -> u64 {
        self.limits.hard_save_bytes.unwrap_or(if self.native_tick {
            2 * 1024 * 1024
        } else {
            crate::saves::MAX_SAVE as u64
        })
    }

    pub fn uses_service_runtime(&self) -> bool {
        self.native_services
    }

    /// Number of ordered read-only directory grants requested by the native runtime.
    pub fn required_game_data_directory_count(&self) -> usize {
        if !self.uses_service_runtime() {
            return 0;
        }
        self.game_data
            .as_ref()
            .and_then(|data| data.layouts.first())
            .map_or(0, |layout| layout.directories.len())
    }

    pub fn uses_selected_servers(&self) -> bool {
        self.game_data
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
        match m.legacy_entry_world.take().as_deref() {
            None | Some("game") => {}
            Some("game-native-v1") => m.native_services = true,
            Some(_) => bail!("invalid legacy entry_world"),
        }
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
        if let Some(options) = m.launch_options {
            options.validate()?;
        }
        anyhow::ensure!(
            !m.native_tick
                || (m.builtin_engine.is_none()
                    && !m.requires_rom
                    && m.game_data.is_none()
                    && !m.permissions.network
                    && m.shared_assets.is_empty()
                    && m.runtime_profile == "core-v1"),
            "native tick packages require dependency-free core game source and embedded assets"
        );
        if let Some(engine) = m.builtin_engine.as_deref() {
            anyhow::ensure!(
                !engine.is_empty()
                    && engine.len() <= 128
                    && engine
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
                "invalid builtin engine"
            );
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
        if m.version.len() > 256 {
            bail!("version is too long");
        }
        semver::Version::parse(&m.version)?;
        m.ensure_engine_compatible()?;
        if !matches!(m.runtime_profile.as_str(), "core-v1" | "component-v1") {
            bail!("unsupported runtime profile");
        }
        if m.uses_service_runtime() {
            anyhow::ensure!(
                m.builtin_engine.is_none() && !m.native_tick && m.runtime_profile == "core-v1",
                "the native service entry requires a core-v1 source game"
            );
        }
        match m.native_graphics {
            Some(NativeGraphics::VertexColorV1) => anyhow::ensure!(
                m.uses_service_runtime()
                    && m.shaders.len() == 2
                    && m.shaders.iter().filter(|shader| shader.stage == "vertex").count() == 1
                    && m.shaders.iter().filter(|shader| shader.stage == "fragment").count() == 1,
                "vertex-color-v1 requires a native service package and one vertex and one fragment Slang entry"
            ),
            Some(NativeGraphics::TexturedV1) => {
                anyhow::ensure!(
                    m.uses_service_runtime()
                        && m.shaders.len() == 2
                        && m.shaders.iter().filter(|shader| shader.stage == "vertex").count() == 1
                        && m.shaders.iter().filter(|shader| shader.stage == "fragment").count() == 1,
                    "textured-v1 requires a native service package and one vertex and one fragment Slang entry"
                );
                let vertex = m
                    .shaders
                    .iter()
                    .find(|shader| shader.stage == "vertex")
                    .context("missing textured-v1 vertex entry")?;
                let fragment = m
                    .shaders
                    .iter()
                    .find(|shader| shader.stage == "fragment")
                    .context("missing textured-v1 fragment entry")?;
                anyhow::ensure!(
                    vertex.source == TEXTURED_V1_SHADER_SOURCE
                        && vertex.entry == TEXTURED_V1_VERTEX_ENTRY
                        && fragment.source == TEXTURED_V1_SHADER_SOURCE
                        && fragment.entry == TEXTURED_V1_FRAGMENT_ENTRY,
                    "textured-v1 requires the host-reviewed Slang source and entries"
                );
            }
            Some(NativeGraphics::TexturedV2) => {
                anyhow::ensure!(
                    m.uses_service_runtime()
                        && m.shaders.len() == 2
                        && m.shaders.iter().filter(|shader| shader.stage == "vertex").count() == 1
                        && m.shaders.iter().filter(|shader| shader.stage == "fragment").count() == 1,
                    "textured-v2 requires a native service package and one vertex and one fragment Slang entry"
                );
                let vertex = m
                    .shaders
                    .iter()
                    .find(|shader| shader.stage == "vertex")
                    .context("missing textured-v2 vertex entry")?;
                let fragment = m
                    .shaders
                    .iter()
                    .find(|shader| shader.stage == "fragment")
                    .context("missing textured-v2 fragment entry")?;
                anyhow::ensure!(
                    vertex.source == TEXTURED_V2_SHADER_SOURCE
                        && vertex.entry == TEXTURED_V2_VERTEX_ENTRY
                        && fragment.source == TEXTURED_V2_SHADER_SOURCE
                        && fragment.entry == TEXTURED_V2_FRAGMENT_ENTRY,
                    "textured-v2 requires the host-reviewed Slang source and entries"
                );
            }
            None if m.uses_service_runtime() => anyhow::ensure!(
                m.shaders.is_empty(),
                "native service Slang shaders require a supported native_graphics profile"
            ),
            None => {}
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
        if m.limits.hard_memory_bytes == 0
            || m.limits.hard_memory_bytes
                > if m.uses_service_runtime() {
                    4 * 1024 * 1024 * 1024
                } else {
                    512 * 1024 * 1024
                }
        {
            bail!("invalid resource limits");
        }
        if let Some(profile) = m.limits.render_profile {
            if m.builtin_engine.is_some()
                || m.limits.hard_memory_bytes < profile.minimum_memory_bytes()
                || m.limits
                    .recommended_memory_bytes
                    .is_some_and(|n| n < profile.minimum_memory_bytes())
            {
                bail!("large-images-v1 requires a source game and at least 256 MiB hard/recommended memory");
            }
        }
        if m.limits.recommended_storage_bytes == Some(0) {
            bail!("recommended storage must be positive");
        }
        if let Some(bytes) = m.limits.hard_save_bytes {
            if !m.permissions.save
                || !(m.native_tick || m.uses_service_runtime())
                || !(1..=MAX_NATIVE_SAVE_BYTES).contains(&bytes)
            {
                bail!("hard save bytes requires native compatibility, save permission and a limit between 1 byte and 64 MiB");
            }
        }
        if let Some(bytes) = m.limits.effects_memory_bytes {
            if !(65536..=4 * 1048576).contains(&bytes)
                || bytes
                    .checked_add(65536)
                    .is_none_or(|n| n > m.limits.hard_memory_bytes)
                || !m.permissions.audio
                || m.builtin_engine.is_some()
            {
                bail!("effects memory requires audio permission, 64 KiB to 4 MiB inside the hard limit, and a source game");
            }
        }
        if let Some(bytes) = m.limits.music_memory_bytes {
            if !(65536..=16 * 1048576).contains(&bytes)
                || !m.permissions.audio
                || m.builtin_engine.is_some()
                || bytes
                    .checked_add(m.limits.effects_memory_bytes.unwrap_or(0))
                    .and_then(|n| n.checked_add(65536))
                    .is_none_or(|n| n > m.limits.hard_memory_bytes)
            {
                bail!("music memory requires audio permission and space inside the source game's hard limit");
            }
        }
        if let Some(recommended) = m.limits.recommended_memory_bytes {
            if recommended == 0 || recommended > m.limits.hard_memory_bytes {
                bail!(
                    "recommended memory must be positive and no greater than the hard memory limit"
                );
            }
        }
        if let Some(data) = &m.game_data {
            anyhow::ensure!(
                m.uses_service_runtime(),
                "game-data folders require native services"
            );
            crate::game_data::validate_native_layout(data)?;
        }
        crate::user_files::validate(&m.permissions.files)?;
        if !m.permissions.files.is_empty() && !m.permissions.input {
            bail!("user-file dialogs require input permission");
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
            bail!("shared assets exceed the shared-pool budget");
        }
        Ok(m)
    }
}

impl Limits {
    pub fn ensure_render_memory(&self, effective_memory: u64) -> Result<()> {
        if let Some(profile) = self.render_profile {
            if effective_memory < profile.minimum_memory_bytes() {
                bail!("large-images-v1 requires a memory ceiling of at least 256 MiB");
            }
        }
        Ok(())
    }
    pub fn web_asset_bytes(&self) -> usize {
        self.render_profile
            .map_or(32 * 1048576, RenderProfile::web_asset_bytes)
    }
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

#[cfg(test)]
mod native_services_tests {
    use super::*;

    #[test]
    fn entry_world_is_read_only_legacy_metadata() {
        let current = include_str!("../examples/minimal/manifest.toml");
        let old = format!("entry_world = \"game\"\n{current}");
        for text in [current, &old] {
            let parsed = Manifest::parse(text.as_bytes()).unwrap();
            assert!(!parsed.uses_service_runtime());
            assert!(!toml::to_string(&parsed).unwrap().contains("entry_world"));
        }
        let standalone = format!("entry_world = \"game-native-v1\"\n{current}");
        let serialized = toml::to_string(&Manifest::parse(standalone.as_bytes()).unwrap()).unwrap();
        assert!(serialized.contains("native_services = true"));
        assert!(!serialized.contains("entry_world"));
        assert!(Manifest::parse(serialized.as_bytes())
            .unwrap()
            .uses_service_runtime());
        let invalid = format!("entry_world = \"unknown\"\n{current}");
        assert!(Manifest::parse(invalid.as_bytes()).is_err());
    }

    #[test]
    fn native_service_rom_requirements_preserve_manifest_order() {
        use std::fmt::Write;

        let base = include_str!("../examples/minimal/manifest.toml");
        let mut native = format!("native_services = true\n{base}");
        for i in 0..8 {
            write!(native, "\n[[roms]]\nid = \"rom{i}\"\ntitle = \"ROM {i}\"\n").unwrap();
        }
        let manifest = Manifest::parse(native.as_bytes()).unwrap();
        assert_eq!(manifest.required_rom_count(), 8);
        assert_eq!(
            manifest
                .roms
                .iter()
                .map(|rom| rom.id.as_str())
                .collect::<Vec<_>>(),
            ["rom0", "rom1", "rom2", "rom3", "rom4", "rom5", "rom6", "rom7"]
        );
        native.push_str("\n[[roms]]\nid = \"rom8\"\ntitle = \"ROM 8\"\n");
        assert!(Manifest::parse(native.as_bytes()).is_err());
    }

    #[test]
    fn hard_save_quota_requires_native_save_permission_and_a_bounded_value() {
        let base = include_str!("../examples/minimal/manifest.toml")
            .replace("save = false", "save = true");
        let native = format!("native_services = true\n{base}");
        assert_eq!(
            Manifest::parse(native.as_bytes())
                .unwrap()
                .native_save_quota(),
            1 << 20
        );
        let tick = base.replace("[limits]", "native_tick = true\n\n[limits]");
        assert_eq!(
            Manifest::parse(tick.as_bytes())
                .unwrap()
                .native_save_quota(),
            2 << 20
        );
        for limit in [1u64, 8 << 20, 64 << 20] {
            let requested =
                native.replace("[limits]", &format!("[limits]\nhard_save_bytes = {limit}"));
            let manifest = Manifest::parse(requested.as_bytes()).unwrap();
            assert_eq!(manifest.native_save_quota(), limit);
            let serialized = toml::to_string(&manifest).unwrap();
            assert_eq!(
                Manifest::parse(serialized.as_bytes())
                    .unwrap()
                    .native_save_quota(),
                limit
            );
            assert!(
                Manifest::parse(requested.replace("save = true", "save = false").as_bytes())
                    .is_err()
            );
            assert!(
                Manifest::parse(requested.replace("native_services = true\n", "").as_bytes())
                    .is_err()
            );
        }
        for invalid in [0, 64 * 1024 * 1024 + 1] {
            assert!(Manifest::parse(
                native
                    .replace(
                        "[limits]",
                        &format!("[limits]\nhard_save_bytes = {invalid}")
                    )
                    .as_bytes()
            )
            .is_err());
        }
    }

    #[test]
    fn service_native_memory_ceilings_preserve_the_wasm_bound() {
        let base = include_str!("../examples/minimal/manifest.toml");
        let native = format!("native_services = true\n{base}");
        for bytes in [2u64 << 30, 4u64 << 30] {
            let manifest = native.replace("16777216", &bytes.to_string());
            assert_eq!(
                Manifest::parse(manifest.as_bytes())
                    .unwrap()
                    .limits
                    .hard_memory_bytes,
                bytes
            );
            assert!(
                Manifest::parse(base.replace("16777216", &bytes.to_string()).as_bytes()).is_err()
            );
        }
        assert!(Manifest::parse(native.replace("16777216", "4294967297").as_bytes()).is_err());
    }

    #[test]
    fn indexed_native_profile_requires_exactly_two_slang_raster_stages() {
        let original = include_str!("../examples/minimal/manifest.toml");
        let base =
            format!("native_services = true\nnative_graphics = \"vertex-color-v1\"\n{original}");
        let shaders = r#"
[[shaders]]
source = "shaders/scene.slang"
entry = "vertexMain"
stage = "vertex"

[[shaders]]
source = "shaders/scene.slang"
entry = "fragmentMain"
stage = "fragment"
"#;
        let valid = format!("{base}{shaders}");
        assert_eq!(
            Manifest::parse(valid.as_bytes()).unwrap().native_graphics,
            Some(NativeGraphics::VertexColorV1)
        );
        assert!(Manifest::parse(base.as_bytes()).is_err());
        assert!(Manifest::parse(
            valid
                .replace("stage = \"fragment\"", "stage = \"compute\"")
                .as_bytes()
        )
        .is_err());
        assert!(Manifest::parse(
            valid
                .replace("native_services = true", "native_services = false")
                .as_bytes()
        )
        .is_err());
    }

    #[test]
    fn textured_native_profile_requires_the_host_reviewed_stages() {
        let original = include_str!("../examples/minimal/manifest.toml");
        let base = format!("native_services = true\nnative_graphics = \"textured-v1\"\n{original}");
        let shaders = format!(
            r#"
[[shaders]]
source = "{}"
entry = "{}"
stage = "vertex"

[[shaders]]
source = "{}"
entry = "{}"
stage = "fragment"
"#,
            TEXTURED_V1_SHADER_SOURCE,
            TEXTURED_V1_VERTEX_ENTRY,
            TEXTURED_V1_SHADER_SOURCE,
            TEXTURED_V1_FRAGMENT_ENTRY
        );
        let valid = format!("{base}{shaders}");
        assert_eq!(
            Manifest::parse(valid.as_bytes()).unwrap().native_graphics,
            Some(NativeGraphics::TexturedV1)
        );
        assert!(Manifest::parse(base.as_bytes()).is_err());
        assert!(Manifest::parse(
            valid
                .replace(TEXTURED_V1_SHADER_SOURCE, "shaders/scene.slang")
                .as_bytes()
        )
        .is_err());
        assert!(Manifest::parse(
            valid
                .replace(TEXTURED_V1_FRAGMENT_ENTRY, "sceneMain")
                .as_bytes()
        )
        .is_err());
        assert!(Manifest::parse(
            valid
                .replace("native_services = true", "native_services = false")
                .as_bytes()
        )
        .is_err());
        assert!(toml::to_string(&Manifest::parse(valid.as_bytes()).unwrap())
            .unwrap()
            .contains("native_graphics = \"textured-v1\""));
    }

    #[test]
    fn textured_v2_native_profile_requires_the_host_reviewed_stages() {
        let original = include_str!("../examples/minimal/manifest.toml");
        let base = format!("native_services = true\nnative_graphics = \"textured-v2\"\n{original}");
        let shaders = format!(
            r#"
[[shaders]]
source = "{}"
entry = "{}"
stage = "vertex"

[[shaders]]
source = "{}"
entry = "{}"
stage = "fragment"
"#,
            TEXTURED_V2_SHADER_SOURCE,
            TEXTURED_V2_VERTEX_ENTRY,
            TEXTURED_V2_SHADER_SOURCE,
            TEXTURED_V2_FRAGMENT_ENTRY
        );
        let valid = format!("{base}{shaders}");
        assert_eq!(
            Manifest::parse(valid.as_bytes()).unwrap().native_graphics,
            Some(NativeGraphics::TexturedV2)
        );
        assert!(Manifest::parse(base.as_bytes()).is_err());
        assert!(Manifest::parse(
            valid
                .replace(TEXTURED_V2_SHADER_SOURCE, "shaders/scene.slang")
                .as_bytes()
        )
        .is_err());
        assert!(Manifest::parse(
            valid
                .replace(TEXTURED_V2_FRAGMENT_ENTRY, "sceneMain")
                .as_bytes()
        )
        .is_err());
        assert!(Manifest::parse(
            valid
                .replace(TEXTURED_V2_SHADER_SOURCE, TEXTURED_V1_SHADER_SOURCE)
                .as_bytes()
        )
        .is_err());
        assert!(Manifest::parse(
            valid
                .replace("native_services = true", "native_services = false")
                .as_bytes()
        )
        .is_err());
        assert!(toml::to_string(&Manifest::parse(valid.as_bytes()).unwrap())
            .unwrap()
            .contains("native_graphics = \"textured-v2\""));
    }
}

#[cfg(test)]
mod serde_compat_tests {
    use super::*;

    #[test]
    fn unknown_top_level_fields_are_ignored_but_nested_tables_stay_strict() {
        let base = include_str!("../examples/minimal/manifest.toml");
        let scalar = format!("future_top_level_field = \"ignored\"\n{base}");
        let table = format!("{base}\n[future_top_level]\nkey = \"ignored\"\n");
        for accepted in [scalar, table] {
            let parsed = Manifest::parse(accepted.as_bytes()).unwrap();
            let serialized = toml::to_string(&parsed).unwrap();
            assert!(!serialized.contains("future_top_level"));
        }

        let nested = base.replace("[limits]", "[limits]\nfuture_nested_field = true");
        assert!(Manifest::parse(nested.as_bytes()).is_err());
    }
}
