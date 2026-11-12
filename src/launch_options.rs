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
#[cfg(any(target_os = "linux", windows, test))]
pub(crate) fn effective(root: &Path, manifest: &Manifest) -> Result<Option<LaunchOptions>> {
    if manifest.builtin_engine.as_deref() != Some(crate::takedawn::PROFILE) {
        return Ok(None);
    }
    Ok(load(root, manifest)?.or(manifest.launch_options))
}

macro_rules! choices {
    ($name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, clap::ValueEnum)]
        pub enum $name { $(#[serde(rename = $text)] #[value(name = $text)] $variant),+ }
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
            (1..=4).contains(&self.menu_hud_scale),
            "menu/HUD scale must be 1, 2, 3 or 4"
        );
        ensure!(
            self.sharpening_percent <= 100,
            "sharpening must be 0–100 percent"
        );
        Ok(())
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
            ("menu_hud_scale = 2", "menu_hud_scale = 5"),
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
    #[derive(clap::Parser)]
    struct Cli {
        #[command(flatten)]
        launch: LaunchArgs,
    }
    fn parse(args: &[&str]) -> LaunchArgs {
        use clap::Parser;
        Cli::try_parse_from(std::iter::once("test").chain(args.iter().copied()))
            .unwrap()
            .launch
    }
    #[test]
    fn public_arguments_roundtrip_all_graphics_combinations() {
        use clap::Parser;
        for &window_mode in WindowMode::ALL {
            for &aspect_ratio in AspectRatio::ALL {
                for &antialiasing in Antialiasing::ALL {
                    for &texture_lod in TextureLod::ALL {
                        for render_scale in 1..=4 {
                            for menu_hud_scale in 1..=4 {
                                for sharpening_percent in [0, 10, 100] {
                                    for widescreen_hud in [false, true] {
                                        let value = LaunchOptions {
                                            window_mode,
                                            aspect_ratio,
                                            antialiasing,
                                            texture_lod,
                                            render_scale,
                                            menu_hud_scale,
                                            sharpening_percent,
                                            widescreen_hud,
                                        };
                                        let args = LaunchArgs::from(value).arguments();
                                        assert_eq!(args.len(), 16);
                                        let parsed = Cli::try_parse_from(
                                            std::iter::once("test".to_owned()).chain(args.clone()),
                                        )
                                        .unwrap()
                                        .launch;
                                        assert_eq!(parsed.arguments(), args);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn per_launch_overrides_preserve_unspecified_and_saved_choices() {
        let saved = LaunchOptions {
            render_scale: 4,
            ..LaunchOptions::default()
        };
        let args = parse(&[
            "--fullscreen",
            "--render-scale",
            "3",
            "--texture-lod=-1",
            "--widescreen-hud",
            "false",
        ]);
        let resolved = args.resolved(Some(saved)).unwrap();
        assert_eq!(resolved.window_mode, Some(WindowMode::Fullscreen));
        assert_eq!(resolved.render_scale, Some(3));
        assert_eq!(resolved.texture_lod, Some(TextureLod::One));
        assert_eq!(resolved.widescreen_hud, Some(false));
        assert_eq!(resolved.antialiasing, Some(saved.antialiasing));
        assert_eq!(saved.render_scale, 4);
        assert!(LaunchArgs::default().resolved(None).unwrap().is_empty());
        assert_eq!(
            parse(&["--windowed"]).resolved(None).unwrap().arguments(),
            ["--window-mode", "windowed"]
        );
        assert_eq!(parse(&["--widescreen-hud"]).widescreen_hud, Some(true));
    }
    #[test]
    fn rejects_invalid_or_conflicting_launch_arguments() {
        use clap::Parser;
        for args in [
            vec!["--fullscreen", "--windowed"],
            vec!["--fullscreen", "--window-mode", "windowed"],
            vec!["--render-scale", "0"],
            vec!["--render-scale", "5"],
            vec!["--menu-hud-scale", "5"],
            vec!["--sharpening-percent", "101"],
            vec!["--aspect-ratio", "32:9"],
            vec!["--texture-lod", "-100"],
            vec!["--antialiasing", "smaa; touch /tmp/file"],
            vec!["--widescreen-hud", "yes"],
            vec!["--render-scale"],
            vec!["--command", "anything"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("test").chain(args)).is_err());
        }
        assert!(LaunchArgs {
            render_scale: Some(0),
            ..Default::default()
        }
        .resolved(None)
        .is_err());
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

/// Public, per-launch graphics overrides. These are data, never a shell command.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
#[group(multiple = true)]
pub struct LaunchArgs {
    #[arg(long, conflicts_with_all = ["windowed", "window_mode"])]
    pub fullscreen: bool,
    #[arg(long, conflicts_with_all = ["fullscreen", "window_mode"])]
    pub windowed: bool,
    #[arg(long, value_enum)]
    pub window_mode: Option<WindowMode>,
    #[arg(long, value_enum)]
    pub aspect_ratio: Option<AspectRatio>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
    pub render_scale: Option<u8>,
    #[arg(long, value_enum)]
    pub antialiasing: Option<Antialiasing>,
    #[arg(long, value_enum, allow_hyphen_values = true)]
    pub texture_lod: Option<TextureLod>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(0..=100))]
    pub sharpening_percent: Option<u8>,
    #[arg(long, num_args = 0..=1, default_missing_value = "true")]
    pub widescreen_hud: Option<bool>,
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4), help = "Menu/HUD scaling: 1 (off), 2, 3 or 4")]
    pub menu_hud_scale: Option<u8>,
}
impl From<LaunchOptions> for LaunchArgs {
    fn from(v: LaunchOptions) -> Self {
        Self {
            window_mode: Some(v.window_mode),
            aspect_ratio: Some(v.aspect_ratio),
            render_scale: Some(v.render_scale),
            antialiasing: Some(v.antialiasing),
            texture_lod: Some(v.texture_lod),
            sharpening_percent: Some(v.sharpening_percent),
            widescreen_hud: Some(v.widescreen_hud),
            menu_hud_scale: Some(v.menu_hud_scale),
            ..Self::default()
        }
    }
}
impl LaunchArgs {
    pub fn is_empty(self) -> bool {
        self.arguments().is_empty()
    }
    pub fn validate(self) -> Result<()> {
        ensure!(
            !(self.fullscreen && self.windowed)
                && !(self.window_mode.is_some() && (self.fullscreen || self.windowed)),
            "choose one window mode"
        );
        ensure!(
            self.render_scale.is_none_or(|x| (1..=4).contains(&x)),
            "render scale must be 1–4"
        );
        ensure!(
            self.menu_hud_scale.is_none_or(|x| (1..=4).contains(&x)),
            "menu/HUD scale must be 1–4"
        );
        ensure!(
            self.sharpening_percent.is_none_or(|x| x <= 100),
            "sharpening must be 0–100 percent"
        );
        Ok(())
    }
    /// Preserve unspecified values, including legacy in-game preferences when
    /// neither the signed manifest nor the host supplies a default for them.
    pub fn resolved(self, base: Option<LaunchOptions>) -> Result<Self> {
        self.validate()?;
        if let Some(base) = base {
            base.validate()?;
        }
        let mut out = base.map(Self::from).unwrap_or_default();
        out.window_mode = self
            .window_mode
            .or(if self.fullscreen {
                Some(WindowMode::Fullscreen)
            } else if self.windowed {
                Some(WindowMode::Windowed)
            } else {
                None
            })
            .or(out.window_mode);
        macro_rules! merge { ($($field:ident),+) => { $(out.$field = self.$field.or(out.$field);)+ }; }
        merge!(
            aspect_ratio,
            render_scale,
            antialiasing,
            texture_lod,
            sharpening_percent,
            widescreen_hud,
            menu_hud_scale
        );
        Ok(out)
    }
    pub fn arguments(self) -> Vec<String> {
        let mut out = Vec::new();
        let mut push = |name: &str, value: String| {
            out.push(name.into());
            out.push(value);
        };
        let mode = self.window_mode.or(if self.fullscreen {
            Some(WindowMode::Fullscreen)
        } else if self.windowed {
            Some(WindowMode::Windowed)
        } else {
            None
        });
        if let Some(x) = mode {
            push("--window-mode", x.label().into());
        }
        if let Some(x) = self.aspect_ratio {
            push("--aspect-ratio", x.label().into());
        }
        if let Some(x) = self.render_scale {
            push("--render-scale", x.to_string());
        }
        if let Some(x) = self.antialiasing {
            push("--antialiasing", x.label().into());
        }
        if let Some(x) = self.texture_lod {
            push("--texture-lod", x.label().into());
        }
        if let Some(x) = self.sharpening_percent {
            push("--sharpening-percent", x.to_string());
        }
        if let Some(x) = self.widescreen_hud {
            push("--widescreen-hud", x.to_string());
        }
        if let Some(x) = self.menu_hud_scale {
            push("--menu-hud-scale", x.to_string());
        }
        out
    }
}
