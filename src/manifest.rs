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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub doom_wad_modes: Vec<DoomWadMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zelda_video: Option<ZeldaVideo>,
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ZeldaVideo {
    #[serde(default)]
    pub extra_width: u32,
    #[serde(default)]
    pub extend_height: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub no_sprite_limits: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dim_flashes: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub skip_intro_on_keypress: bool,
    #[serde(default, skip_serializing_if = "ZeldaEnhancements::is_empty")]
    pub enhancements: ZeldaEnhancements,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZeldaEnhancements {
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub item_switch_lr: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub item_switch_lr_limit: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub turn_while_dashing: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mirror_to_darkworld: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub collect_items_with_sword: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub break_pots_with_sword: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub disable_low_health_beep: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub show_max_items_in_yellow: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub more_active_bombs: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub carry_more_rupees: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub misc_bug_fixes: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub game_changing_bug_fixes: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cancel_bird_travel: bool,
}
impl ZeldaEnhancements {
    pub const CHOICES: &'static [(&'static str, &'static str)] = &[
        ("item_switch_lr", "Cycle items with shoulder buttons"),
        (
            "item_switch_lr_limit",
            "Limit shoulder cycling to the first four slots",
        ),
        ("turn_while_dashing", "Turn while dashing"),
        (
            "mirror_to_darkworld",
            "Allow mirror travel to the Dark World",
        ),
        ("collect_items_with_sword", "Collect items with the sword"),
        ("break_pots_with_sword", "Break pots with the sword"),
        ("disable_low_health_beep", "Disable the low-health beep"),
        (
            "show_max_items_in_yellow",
            "Highlight maximum item counts in yellow",
        ),
        ("more_active_bombs", "Allow more active bombs"),
        ("carry_more_rupees", "Carry up to 9,999 rupees"),
        ("misc_bug_fixes", "Original port's miscellaneous bug fixes"),
        (
            "game_changing_bug_fixes",
            "Original port's gameplay-changing bug fixes",
        ),
        ("cancel_bird_travel", "Allow canceling bird travel"),
    ];
    pub fn enabled(&self, name: &str) -> Option<bool> {
        match name {
            "item_switch_lr" => Some(self.item_switch_lr),
            "item_switch_lr_limit" => Some(self.item_switch_lr_limit),
            "turn_while_dashing" => Some(self.turn_while_dashing),
            "mirror_to_darkworld" => Some(self.mirror_to_darkworld),
            "collect_items_with_sword" => Some(self.collect_items_with_sword),
            "break_pots_with_sword" => Some(self.break_pots_with_sword),
            "disable_low_health_beep" => Some(self.disable_low_health_beep),
            "show_max_items_in_yellow" => Some(self.show_max_items_in_yellow),
            "more_active_bombs" => Some(self.more_active_bombs),
            "carry_more_rupees" => Some(self.carry_more_rupees),
            "misc_bug_fixes" => Some(self.misc_bug_fixes),
            "game_changing_bug_fixes" => Some(self.game_changing_bug_fixes),
            "cancel_bird_travel" => Some(self.cancel_bird_travel),
            _ => None,
        }
    }
    pub fn set(&mut self, name: &str, enabled: bool) -> Result<()> {
        match name {
            "item_switch_lr" => self.item_switch_lr = enabled,
            "item_switch_lr_limit" => self.item_switch_lr_limit = enabled,
            "turn_while_dashing" => self.turn_while_dashing = enabled,
            "mirror_to_darkworld" => self.mirror_to_darkworld = enabled,
            "collect_items_with_sword" => self.collect_items_with_sword = enabled,
            "break_pots_with_sword" => self.break_pots_with_sword = enabled,
            "disable_low_health_beep" => self.disable_low_health_beep = enabled,
            "show_max_items_in_yellow" => self.show_max_items_in_yellow = enabled,
            "more_active_bombs" => self.more_active_bombs = enabled,
            "carry_more_rupees" => self.carry_more_rupees = enabled,
            "misc_bug_fixes" => self.misc_bug_fixes = enabled,
            "game_changing_bug_fixes" => self.game_changing_bug_fixes = enabled,
            "cancel_bird_travel" => self.cancel_bird_travel = enabled,
            _ => bail!("unknown Zelda enhancement: {name}"),
        }
        Ok(())
    }
    pub fn bits(&self) -> u32 {
        Self::CHOICES
            .iter()
            .enumerate()
            .fold(0, |bits, (index, (name, _))| {
                bits | (u32::from(self.enabled(name).expect("reviewed enhancement name")) << index)
            })
    }
    pub fn is_empty(&self) -> bool {
        self.bits() == 0
    }
}
impl ZeldaVideo {
    pub fn configuration_bits(self) -> u32 {
        u32::from(self.extend_height)
            | (u32::from(self.no_sprite_limits) << 1)
            | (u32::from(self.dim_flashes) << 2)
            | (u32::from(self.skip_intro_on_keypress) << 3)
            | (self.enhancements.bits() << 4)
    }
    pub fn dimensions(self) -> (u32, u32) {
        (
            256 + self.extra_width * 2,
            if self.extend_height { 240 } else { 224 },
        )
    }
    pub fn validate(self) -> Result<()> {
        if self.extra_width > 96 {
            bail!("Zelda extra_width must be between 0 and 96 pixels per side");
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u32)]
pub enum DoomWadMode {
    File = 0,
    Merge = 1,
    NwtMerge = 2,
    Flats = 3,
    Sprites = 4,
    Both = 5,
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
    /// Host-owned recorded effects share, rather than extend, the hard limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects_memory_bytes: Option<u64>,
    /// Host-owned asynchronous music shares, rather than extends, the hard limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub music_memory_bytes: Option<u64>,
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
        if !m.doom_wad_modes.is_empty()
            && (m.builtin_engine.as_deref() != Some(crate::doom::PROFILE)
                || m.doom_wad_modes.len() != m.required_rom_count().saturating_sub(1))
        {
            bail!("DOOM WAD modes require one mode per declared patch in the pinned DOOM recipe");
        }
        if let Some(video) = m.zelda_video {
            if m.builtin_engine.as_deref() != Some(crate::zelda3::PROFILE) {
                bail!("Zelda video options require the pinned Zelda recipe");
            }
            video.validate()?;
        }
        let cube2 = m.builtin_engine.as_deref() == Some(crate::cube2::PROFILE);
        let takedawn = m.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE);
        if takedawn {
            if m.runtime_profile != "core-v1"
                || m.entry_world != "game"
                || m.required_rom_count() != 1
                || m.game_data.is_some()
                || !m.permissions.save
                || !m.permissions.audio
                || !m.permissions.input
                || m.permissions.network
                || !m.permissions.files.is_empty()
                || !m.shaders.is_empty()
                || !m.shared_assets.is_empty()
                || m.limits.hard_memory_bytes != crate::takedawn::MEMORY
            {
                bail!("Takedawn requires its native recipe, one user ISO, input/audio/save permissions and 4 GiB memory");
            }
        } else if cube2 {
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
                    || if engine == crate::doom::PROFILE {
                        !(1..=7).contains(&m.required_rom_count())
                    } else {
                        m.required_rom_count() != 1
                    }
                    || !m.permissions.save
                    || !m.permissions.audio
                    || m.limits.hard_memory_bytes < minimum
                {
                    bail!("builtin games require a pinned core-v1 engine, one base ROM (DOOM may declare up to six additional patch WADs), save/audio permissions and enough memory for the engine plus 16 MiB");
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
                > if cube2 || takedawn {
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
