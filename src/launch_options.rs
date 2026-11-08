//! Signed graphics defaults and host-owned per-game launch preferences.
use crate::manifest::Manifest;
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, io::Write, path::Path};
const MAX_BYTES: u64 = 1048576;
const FILE: &str = "launch-selections.json";
pub fn identity(manifest: &Manifest) -> Result<String> {
    ensure!(
        manifest.builtin_engine.as_deref() == Some(crate::takedawn::PROFILE),
        "launch preferences require the reviewed Takedawn engine"
    );
    Ok(crate::verify::digest(&serde_json::to_vec(&(
        &manifest.id,
        &manifest.creator_key,
        &manifest.builtin_engine,
    ))?))
}
fn read(root: &Path) -> Result<BTreeMap<String, LaunchOptions>> {
    let bytes = match crate::files::read_regular(&root.join(FILE), MAX_BYTES, false) {
        Ok(bytes) => bytes,
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(BTreeMap::new())
        }
        Err(error) => return Err(error),
    };
    let entries: BTreeMap<String, LaunchOptions> = serde_json::from_slice(&bytes)?;
    ensure!(entries.len() <= 1024, "too many remembered launch choices");
    for (key, options) in &entries {
        ensure!(
            key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid launch choice identity"
        );
        options.validate()?;
    }
    Ok(entries)
}
pub fn load(root: &Path, manifest: &Manifest) -> Result<Option<LaunchOptions>> {
    let key = identity(manifest)?;
    Ok(read(root)?.remove(&key))
}
pub(crate) fn remember(
    root: &Path,
    manifest: &Manifest,
    options: Option<LaunchOptions>,
) -> Result<()> {
    let key = identity(manifest)?;
    if let Some(options) = options {
        options.validate()?;
    }
    let mut entries = read(root)?;
    if let Some(options) = options {
        entries.insert(key, options);
    } else {
        entries.remove(&key);
    }
    ensure!(entries.len() <= 1024, "too many remembered launch choices");
    let bytes = serde_json::to_vec_pretty(&entries)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "launch choices exceed their quota"
    );
    let metadata = root.symlink_metadata()?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "library must be a real directory"
    );
    let mut temporary = tempfile::NamedTempFile::new_in(root)?;
    temporary.write_all(&bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(root.join(FILE))
        .map_err(|error| error.error)?;
    Ok(())
}
pub fn remember_for_game(root: &Path, id: &str, options: Option<LaunchOptions>) -> Result<()> {
    let _change = crate::registry::LibraryChange::acquire(root)?;
    let manifest = crate::registry::installed_manifest(root, id)?;
    remember(root, &manifest, options)
}
pub(crate) fn effective(root: &Path, manifest: &Manifest) -> Result<Option<LaunchOptions>> {
    if manifest.builtin_engine.as_deref() != Some(crate::takedawn::PROFILE) {
        return Ok(None);
    }
    Ok(load(root, manifest)?.or(manifest.launch_options))
}

macro_rules! choices {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        pub enum $name { $(#[serde(rename = $text)] $variant),+ }
        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];
            pub fn label(self) -> &'static str { match self { $(Self::$variant => $text),+ } }
        }
    };
}
choices!(WindowMode { Windowed => "windowed", Fullscreen => "fullscreen" });
choices!(AspectRatio { Standard => "4:3", Wide => "16:9" });
choices!(Antialiasing { Off => "off", Fxaa => "fxaa", Smaa => "smaa" });
choices!(TextureLod { Off => "off", Auto => "auto", Half => "-0.5", One => "-1", OneHalf => "-1.5" });

/// Fixed renderer settings only: no command lines, executable paths or arbitrary environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LaunchOptions {
    pub window_mode: WindowMode,
    pub aspect_ratio: AspectRatio,
    pub render_scale: u8,
    pub antialiasing: Antialiasing,
    pub texture_lod: TextureLod,
    pub sharpening_percent: u8,
    pub widescreen_hud: bool,
    pub menu_hud_scale: u8,
}
impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            window_mode: WindowMode::Windowed,
            aspect_ratio: AspectRatio::Wide,
            render_scale: 2,
            antialiasing: Antialiasing::Smaa,
            texture_lod: TextureLod::Auto,
            sharpening_percent: 10,
            widescreen_hud: true,
            menu_hud_scale: 2,
        }
    }
}
impl LaunchOptions {
    pub fn validate(self) -> Result<()> {
        ensure!(
            (1..=4).contains(&self.render_scale),
            "render scale must be 1, 2, 3 or 4"
        );
        ensure!(
            [1, 2, 4].contains(&self.menu_hud_scale),
            "menu/HUD scale must be 1, 2 or 4"
        );
        ensure!(
            self.sharpening_percent <= 100,
            "sharpening must be 0–100 percent"
        );
        Ok(())
    }
    pub(crate) fn environment(self) -> Vec<(&'static str, String)> {
        vec![
            (
                "RECOMP_PRESENT_FULLSCREEN",
                if self.window_mode == WindowMode::Fullscreen {
                    "1"
                } else {
                    "0"
                }
                .into(),
            ),
            (
                "RECOMP_DISPLAY_WIDESCREEN",
                if self.aspect_ratio == AspectRatio::Wide {
                    "1"
                } else {
                    "0"
                }
                .into(),
            ),
            ("RECOMP_RENDER_SCALE", self.render_scale.to_string()),
            ("RECOMP_GFX_AA", self.antialiasing.label().into()),
            (
                "RECOMP_GFX_LOD_BIAS",
                if self.texture_lod == TextureLod::Off {
                    "0"
                } else {
                    self.texture_lod.label()
                }
                .into(),
            ),
            (
                "RECOMP_GFX_SHARPEN",
                format!("{:.2}", f64::from(self.sharpening_percent) / 100.0),
            ),
            (
                "RECOMP_GFX_HUD_FIX",
                if self.widescreen_hud { "1" } else { "0" }.into(),
            ),
            (
                "RECOMP_GFX_UI_SCALE",
                if self.menu_hud_scale == 1 {
                    "off".into()
                } else {
                    format!("{}x", self.menu_hud_scale)
                },
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn game() -> Manifest {
        Manifest::parse(include_bytes!("../ports/takedawn/manifest.toml")).unwrap()
    }
    #[test]
    fn signed_manifest_roundtrip_and_invalid_launch_settings() {
        let mut manifest = game();
        manifest.launch_options = Some(LaunchOptions::default());
        let text = toml::to_string(&manifest).unwrap();
        assert_eq!(
            Manifest::parse(text.as_bytes()).unwrap().launch_options,
            manifest.launch_options
        );
        for (from, to) in [
            ("render_scale = 2", "render_scale = 0"),
            ("render_scale = 2", "render_scale = 5"),
            ("menu_hud_scale = 2", "menu_hud_scale = 3"),
            ("sharpening_percent = 10", "sharpening_percent = 101"),
            ("aspect_ratio = \"16:9\"", "aspect_ratio = \"32:9\""),
            (
                "antialiasing = \"smaa\"",
                "antialiasing = \"smaa; touch /tmp/file\"",
            ),
            ("texture_lod = \"auto\"", "texture_lod = \"-100\""),
            (
                "window_mode = \"windowed\"",
                "window_mode = \"fullscreen\"\ncommand = \"anything\"",
            ),
        ] {
            assert!(text.contains(from), "{from}");
            assert!(
                Manifest::parse(text.replace(from, to).as_bytes()).is_err(),
                "{to}"
            );
        }
        let mut other = Manifest::parse(include_bytes!("../ports/doom/manifest.toml")).unwrap();
        other.launch_options = manifest.launch_options;
        assert!(Manifest::parse(toml::to_string(&other).unwrap().as_bytes()).is_err());
    }
    #[test]
    fn defaults_and_original_settings_map_to_existing_runtime_controls() {
        let default = LaunchOptions::default();
        let env: BTreeMap<_, _> = default.environment().into_iter().collect();
        for (key, value) in [
            ("RECOMP_PRESENT_FULLSCREEN", "0"),
            ("RECOMP_DISPLAY_WIDESCREEN", "1"),
            ("RECOMP_RENDER_SCALE", "2"),
            ("RECOMP_GFX_AA", "smaa"),
            ("RECOMP_GFX_LOD_BIAS", "auto"),
            ("RECOMP_GFX_SHARPEN", "0.10"),
            ("RECOMP_GFX_HUD_FIX", "1"),
            ("RECOMP_GFX_UI_SCALE", "2x"),
        ] {
            assert_eq!(env[key], value);
        }
        let original = LaunchOptions {
            window_mode: WindowMode::Fullscreen,
            aspect_ratio: AspectRatio::Standard,
            render_scale: 1,
            antialiasing: Antialiasing::Off,
            texture_lod: TextureLod::Off,
            sharpening_percent: 0,
            widescreen_hud: false,
            menu_hud_scale: 1,
        };
        original.validate().unwrap();
        let env: BTreeMap<_, _> = original.environment().into_iter().collect();
        assert_eq!(env["RECOMP_PRESENT_FULLSCREEN"], "1");
        assert_eq!(env["RECOMP_DISPLAY_WIDESCREEN"], "0");
        assert_eq!(env["RECOMP_GFX_LOD_BIAS"], "0");
        assert_eq!(env["RECOMP_GFX_UI_SCALE"], "off");
        // Every exposed combination uses a fixed key and a bounded value.
        for &aa in Antialiasing::ALL {
            for &lod in TextureLod::ALL {
                for scale in 1..=4 {
                    let value = LaunchOptions {
                        antialiasing: aa,
                        texture_lod: lod,
                        render_scale: scale,
                        ..default
                    };
                    value.validate().unwrap();
                    assert!(value
                        .environment()
                        .iter()
                        .all(|(_, v)| v.len() <= 10 && !v.contains(['\n', '\0', ';'])));
                }
            }
        }
    }
    #[test]
    fn overrides_follow_creator_game_engine_and_reset_to_updated_defaults() {
        let root = tempfile::tempdir().unwrap();
        let mut manifest = game();
        manifest.launch_options = Some(LaunchOptions::default());
        let selected = LaunchOptions {
            render_scale: 3,
            ..LaunchOptions::default()
        };
        remember(root.path(), &manifest, Some(selected)).unwrap();
        manifest.version = "99.0.0".into();
        manifest.launch_options.as_mut().unwrap().render_scale = 4;
        assert_eq!(effective(root.path(), &manifest).unwrap(), Some(selected));
        let mut other = manifest.clone();
        other.creator_key.push('a');
        assert_eq!(load(root.path(), &other).unwrap(), None);
        other = manifest.clone();
        other.id.push_str(".other");
        assert_eq!(load(root.path(), &other).unwrap(), None);
        remember(root.path(), &manifest, None).unwrap();
        assert_eq!(
            effective(root.path(), &manifest).unwrap(),
            manifest.launch_options
        );
        std::fs::write(root.path().join(FILE), b"broken").unwrap();
        assert!(remember(root.path(), &manifest, Some(selected)).is_err());
        assert_eq!(std::fs::read(root.path().join(FILE)).unwrap(), b"broken");
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_preferences_are_rejected_without_touching_the_target() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(outside.path(), b"{}").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join(FILE)).unwrap();
        assert!(load(root.path(), &game()).is_err());
        assert!(remember(root.path(), &game(), None).is_err());
        assert_eq!(std::fs::read(outside.path()).unwrap(), b"{}");
    }
}
