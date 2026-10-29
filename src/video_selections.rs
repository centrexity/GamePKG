//! Host-owned display preferences, scoped to a creator, game and reviewed engine.
use crate::manifest::{Manifest, ZeldaVideo};
use anyhow::{ensure, Result};
use std::{collections::BTreeMap, io::Write, path::Path};
const MAX_BYTES: u64 = 1048576;
const FILE: &str = "video-selections.json";
pub fn identity(manifest: &Manifest) -> Result<String> {
    ensure!(
        manifest.builtin_engine.as_deref() == Some(crate::zelda3::PROFILE),
        "display preferences require the pinned Zelda engine"
    );
    Ok(crate::verify::digest(&serde_json::to_vec(&(
        &manifest.id,
        &manifest.creator_key,
        &manifest.builtin_engine,
    ))?))
}
fn read(root: &Path) -> Result<BTreeMap<String, ZeldaVideo>> {
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
    let entries: BTreeMap<String, ZeldaVideo> = serde_json::from_slice(&bytes)?;
    ensure!(entries.len() <= 1024, "too many remembered display choices");
    for (key, video) in &entries {
        ensure!(
            key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "invalid display choice identity"
        );
        video.validate()?;
    }
    Ok(entries)
}
pub fn load(root: &Path, manifest: &Manifest) -> Result<Option<ZeldaVideo>> {
    let key = identity(manifest)?;
    Ok(read(root)?.remove(&key))
}
pub(crate) fn remember(root: &Path, manifest: &Manifest, video: Option<ZeldaVideo>) -> Result<()> {
    let key = identity(manifest)?;
    if let Some(video) = video {
        video.validate()?;
    }
    let mut entries = read(root)?;
    if let Some(video) = video {
        entries.insert(key, video);
    } else {
        entries.remove(&key);
    }
    ensure!(entries.len() <= 1024, "too many remembered display choices");
    let bytes = serde_json::to_vec_pretty(&entries)?;
    ensure!(
        bytes.len() as u64 <= MAX_BYTES,
        "display choices exceed their quota"
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
pub fn remember_for_game(root: &Path, id: &str, video: Option<ZeldaVideo>) -> Result<()> {
    let _change = crate::registry::LibraryChange::acquire(root)?;
    let manifest = crate::registry::installed_manifest(root, id)?;
    remember(root, &manifest, video)
}
pub(crate) fn effective(root: &Path, manifest: &Manifest) -> Result<Option<ZeldaVideo>> {
    if manifest.builtin_engine.as_deref() != Some(crate::zelda3::PROFILE) {
        return Ok(None);
    }
    Ok(load(root, manifest)?.or(manifest.zelda_video))
}
pub(crate) fn require_engine_support(video: Option<ZeldaVideo>, supported: bool) -> Result<()> {
    let video = video.unwrap_or_default();
    video.validate()?;
    ensure!(
        supported || video == ZeldaVideo::default(),
        "rebuild the Zelda engine to apply display settings"
    );
    Ok(())
}
pub fn wide(extended: bool) -> ZeldaVideo {
    let height = if extended { 240 } else { 224 };
    ZeldaVideo {
        extra_width: (height * 16 / 9 - 256) / 2,
        extend_height: extended,
        no_sprite_limits: false,
        dim_flashes: false,
        skip_intro_on_keypress: false,
        enhancements: Default::default(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        Manifest::parse(include_bytes!("../ports/zelda3/manifest.toml")).unwrap()
    }
    #[test]
    fn overrides_persist_across_updates_and_reset_to_current_package_defaults() {
        let root = tempfile::tempdir().unwrap();
        let mut game = manifest();
        game.zelda_video = Some(ZeldaVideo {
            extra_width: 32,
            extend_height: false,
            no_sprite_limits: false,
            dim_flashes: false,
            skip_intro_on_keypress: false,
            enhancements: Default::default(),
        });
        assert_eq!(effective(root.path(), &game).unwrap(), game.zelda_video);
        let video = Some(ZeldaVideo {
            no_sprite_limits: true,
            dim_flashes: false,
            skip_intro_on_keypress: false,
            enhancements: Default::default(),
            ..wide(true)
        });
        remember(root.path(), &game, video).unwrap();
        game.version = "99.0.0".into();
        game.zelda_video = Some(ZeldaVideo::default());
        assert_eq!(load(root.path(), &game).unwrap(), video);
        assert_eq!(effective(root.path(), &game).unwrap(), video);
        remember(root.path(), &game, None).unwrap();
        assert_eq!(load(root.path(), &game).unwrap(), None);
        assert_eq!(effective(root.path(), &game).unwrap(), game.zelda_video);
    }
    #[test]
    fn creator_game_and_engine_changes_never_inherit_an_override() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        remember(root.path(), &game, Some(wide(false))).unwrap();
        for changed in 0..2 {
            let mut other = game.clone();
            if changed == 0 {
                other.creator_key = "different".into();
            } else {
                other.id = "com.example.other".into();
            }
            assert_eq!(load(root.path(), &other).unwrap(), None);
        }
        let doom = Manifest::parse(include_bytes!("../ports/doom/manifest.toml")).unwrap();
        assert!(remember(root.path(), &doom, Some(wide(false))).is_err());
        std::fs::write(root.path().join(FILE), b"damaged").unwrap();
        assert_eq!(effective(root.path(), &doom).unwrap(), None);
    }
    #[test]
    fn malformed_or_oversized_preferences_fail_without_overwriting() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        let key = identity(&game).unwrap();
        for damaged in [
            b"broken".to_vec(),
            format!("{{\"{key}\":{{\"extra_width\":97}}}}").into_bytes(),
            b"{\"bad-key\":{}}".to_vec(),
            vec![b' '; MAX_BYTES as usize + 1],
        ] {
            std::fs::write(root.path().join(FILE), &damaged).unwrap();
            assert!(load(root.path(), &game).is_err());
            assert!(remember(root.path(), &game, None).is_err());
            assert_eq!(std::fs::read(root.path().join(FILE)).unwrap(), damaged);
        }
    }
    #[test]
    fn all_off_override_is_distinct_from_signed_enhancements_and_package_default() {
        let root = tempfile::tempdir().unwrap();
        let mut game = manifest();
        let mut video = ZeldaVideo::default();
        for &(name, _) in crate::manifest::ZeldaEnhancements::CHOICES {
            video.enhancements.set(name, true).unwrap();
        }
        game.zelda_video = Some(video);
        assert_eq!(effective(root.path(), &game).unwrap(), Some(video));
        remember(root.path(), &game, Some(ZeldaVideo::default())).unwrap();
        assert_eq!(
            effective(root.path(), &game).unwrap(),
            Some(ZeldaVideo::default())
        );
        remember(root.path(), &game, None).unwrap();
        assert_eq!(effective(root.path(), &game).unwrap(), Some(video));
    }
    #[test]
    fn original_enhancements_roundtrip_every_boolean_combination() {
        use crate::manifest::ZeldaEnhancements;
        for bits in 0..(1u32 << ZeldaEnhancements::CHOICES.len()) {
            let mut enhancements = ZeldaEnhancements::default();
            for (index, &(name, _)) in ZeldaEnhancements::CHOICES.iter().enumerate() {
                enhancements.set(name, bits & (1 << index) != 0).unwrap();
            }
            let video = ZeldaVideo {
                enhancements,
                ..Default::default()
            };
            let encoded = serde_json::to_vec(&video).unwrap();
            let restored: ZeldaVideo = serde_json::from_slice(&encoded).unwrap();
            assert_eq!(restored, video);
            assert_eq!(video.configuration_bits(), bits << 4);
            assert_eq!(video.dimensions(), (256, 224));
        }
        let empty = serde_json::to_string(&ZeldaVideo::default()).unwrap();
        assert!(!empty.contains("enhancements"));
    }
    #[test]
    fn invalid_original_enhancement_metadata_preserves_saved_bytes() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        let key = identity(&game).unwrap();
        for invalid in [r#"{"invented":true}"#, r#"{"turn_while_dashing":1}"#] {
            let bytes = format!("{{\"{key}\":{{\"enhancements\":{invalid}}}}}");
            std::fs::write(root.path().join(FILE), bytes.as_bytes()).unwrap();
            assert!(load(root.path(), &game).is_err());
            assert!(remember(root.path(), &game, None).is_err());
            assert_eq!(
                std::fs::read(root.path().join(FILE)).unwrap(),
                bytes.as_bytes()
            );
        }
    }
    #[test]
    fn early_intro_choice_persists_without_changing_other_options() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        let video = ZeldaVideo {
            skip_intro_on_keypress: true,
            enhancements: Default::default(),
            ..ZeldaVideo::default()
        };
        remember(root.path(), &game, Some(video)).unwrap();
        assert_eq!(load(root.path(), &game).unwrap(), Some(video));
        assert_eq!(video.dimensions(), (256, 224));
        assert!(!serde_json::to_string(&ZeldaVideo::default())
            .unwrap()
            .contains("skip_intro_on_keypress"));
        let legacy: ZeldaVideo = serde_json::from_str(r#"{"dim_flashes":true}"#).unwrap();
        assert!(legacy.dim_flashes && !legacy.skip_intro_on_keypress);
    }
    #[test]
    fn dim_flashes_persist_independently_and_legacy_values_default_off() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        let video = ZeldaVideo {
            dim_flashes: true,
            skip_intro_on_keypress: false,
            enhancements: Default::default(),
            ..ZeldaVideo::default()
        };
        remember(root.path(), &game, Some(video)).unwrap();
        assert_eq!(effective(root.path(), &game).unwrap(), Some(video));
        assert_eq!(video.dimensions(), (256, 224));
        assert!(!wide(false).dim_flashes);
        assert!(!serde_json::to_string(&wide(false))
            .unwrap()
            .contains("dim_flashes"));
        let legacy: ZeldaVideo = serde_json::from_str(
            r#"{"extra_width":32,"extend_height":false,"no_sprite_limits":true}"#,
        )
        .unwrap();
        assert!(!legacy.dim_flashes && legacy.no_sprite_limits);
    }
    #[test]
    fn earlier_two_field_preferences_keep_original_sprite_limits() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        let key = identity(&game).unwrap();
        std::fs::write(
            root.path().join(FILE),
            format!("{{\"{key}\":{{\"extra_width\":71,\"extend_height\":false}}}}"),
        )
        .unwrap();
        assert_eq!(load(root.path(), &game).unwrap(), Some(wide(false)));
        assert!(!serde_json::to_string(&wide(false))
            .unwrap()
            .contains("no_sprite_limits"));
    }
    #[test]
    fn remembered_game_count_is_bounded_without_replacing_existing_entries() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        let entries: BTreeMap<String, ZeldaVideo> = (0..1024)
            .map(|index| (format!("{index:064x}"), wide(false)))
            .collect();
        assert!(!entries.contains_key(&identity(&game).unwrap()));
        let before = serde_json::to_vec(&entries).unwrap();
        std::fs::write(root.path().join(FILE), &before).unwrap();
        assert_eq!(load(root.path(), &game).unwrap(), None);
        assert!(remember(root.path(), &game, Some(wide(false))).is_err());
        assert_eq!(std::fs::read(root.path().join(FILE)).unwrap(), before);
    }
    #[test]
    fn default_options_need_no_optional_configuration_but_extensions_do() {
        for video in [None, Some(ZeldaVideo::default())] {
            require_engine_support(video, false).unwrap();
        }
        for video in [
            ZeldaVideo {
                no_sprite_limits: true,
                dim_flashes: false,
                skip_intro_on_keypress: false,
                enhancements: Default::default(),
                ..Default::default()
            },
            wide(false),
            wide(true),
            ZeldaVideo {
                extra_width: 0,
                extend_height: true,
                no_sprite_limits: false,
                dim_flashes: false,
                skip_intro_on_keypress: false,
                enhancements: Default::default(),
            },
        ] {
            assert!(require_engine_support(Some(video), false)
                .unwrap_err()
                .to_string()
                .contains("rebuild"));
            require_engine_support(Some(video), true).unwrap();
        }
        assert!(require_engine_support(
            Some(ZeldaVideo {
                extra_width: 97,
                extend_height: false,
                no_sprite_limits: false,
                dim_flashes: false,
                skip_intro_on_keypress: false,
                enhancements: Default::default()
            }),
            true
        )
        .is_err());
    }
    #[test]
    fn invalid_selection_does_not_replace_a_valid_choice() {
        let root = tempfile::tempdir().unwrap();
        let game = manifest();
        remember(root.path(), &game, Some(wide(false))).unwrap();
        let before = std::fs::read(root.path().join(FILE)).unwrap();
        assert!(remember(
            root.path(),
            &game,
            Some(ZeldaVideo {
                extra_width: u32::MAX,
                extend_height: true,
                no_sprite_limits: false,
                dim_flashes: false,
                skip_intro_on_keypress: false,
                enhancements: Default::default()
            })
        )
        .is_err());
        assert_eq!(std::fs::read(root.path().join(FILE)).unwrap(), before);
        assert_eq!(wide(false).dimensions(), (398, 224));
        assert_eq!(wide(true).dimensions(), (426, 240));
    }
}
