//! Host-owned window settings. Guests never see paths or write these records.
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    version: u32,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub maximized: bool,
    #[serde(default = "default_vsync")]
    pub vsync: bool,
}
fn default_vsync() -> bool {
    true
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            width: 960,
            height: 720,
            fullscreen: false,
            maximized: false,
            vsync: true,
        }
    }
}
impl Preferences {
    fn validate(&self) -> Result<()> {
        if self.version != 1
            || !(320..=10000).contains(&self.width)
            || !(240..=10000).contains(&self.height)
        {
            bail!("invalid game window settings");
        }
        Ok(())
    }
    // Keep the last normal size rather than replacing it with a monitor's size.
    pub fn observe(
        &mut self,
        size: Option<[f32; 2]>,
        fullscreen: Option<bool>,
        maximized: Option<bool>,
        minimized: bool,
    ) {
        if minimized {
            return;
        }
        let previous_fullscreen = self.fullscreen;
        let previous_maximized = self.maximized;
        if let Some(value) = fullscreen {
            self.fullscreen = value;
        }
        if !self.fullscreen {
            if let Some(value) = maximized {
                self.maximized = value;
            }
        }
        // The first frame leaving fullscreen/maximization may still have old bounds.
        if !self.fullscreen && !self.maximized && !previous_fullscreen && !previous_maximized {
            if let Some([width, height]) = size {
                let width = width.round();
                let height = height.round();
                if width.is_finite()
                    && height.is_finite()
                    && (320.0..=10000.0).contains(&width)
                    && (240.0..=10000.0).contains(&height)
                {
                    self.width = width as u32;
                    self.height = height as u32;
                }
            }
        }
    }
}
fn directory(root: &Path) -> Result<PathBuf> {
    let path = root.join("window-preferences");
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            bail!("window settings folder must be a directory, not a link")
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(path)
}
fn read(path: &Path) -> Result<Option<Preferences>> {
    match path.symlink_metadata() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
        Ok(_) => {}
    }
    let record: Preferences =
        serde_json::from_slice(&crate::files::read_regular(path, 256, false)?)?;
    record.validate()?;
    Ok(Some(record))
}
pub(crate) fn path(root: &Path, id: &str, creator: &str) -> PathBuf {
    let identity = crate::verify::digest(format!("GamePKG window v1\0{id}\0{creator}").as_bytes());
    root.join("window-preferences")
        .join(format!("{identity}.json"))
}
pub struct Session {
    path: PathBuf,
    root: PathBuf,
    pub preferences: Preferences,
    saved: Option<Preferences>,
}
impl Session {
    pub fn open(root: &Path, id: &str, creator: &str) -> Result<Self> {
        directory(root)?;
        let path = path(root, id, creator);
        let saved = read(&path)?;
        Ok(Self {
            path,
            root: root.to_owned(),
            preferences: saved.unwrap_or_default(),
            saved,
        })
    }
    pub fn persist(&mut self) -> Result<()> {
        self.preferences.validate()?;
        if self.saved == Some(self.preferences) {
            return Ok(());
        }
        let directory = directory(&self.root)?;
        // Preserve damaged or replaced records rather than silently repairing them.
        read(&self.path)?;
        std::fs::create_dir_all(&directory)?;
        directory.symlink_metadata().and_then(|metadata| {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                Err(std::io::Error::other("window settings folder changed"))
            } else {
                Ok(metadata)
            }
        })?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        temporary.write_all(&serde_json::to_vec(&self.preferences)?)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path)?;
        self.saved = Some(self.preferences);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_window_records_default_to_vsync_and_new_choices_persist_per_game() {
        let root = tempfile::tempdir().unwrap();
        let mut session = Session::open(root.path(), "game", "creator").unwrap();
        session.persist().unwrap();
        let mut old: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&session.path).unwrap()).unwrap();
        old.as_object_mut().unwrap().remove("vsync");
        std::fs::write(&session.path, serde_json::to_vec(&old).unwrap()).unwrap();
        let mut session = Session::open(root.path(), "game", "creator").unwrap();
        assert!(session.preferences.vsync);
        session.preferences.vsync = false;
        session.persist().unwrap();
        assert!(
            !Session::open(root.path(), "game", "creator")
                .unwrap()
                .preferences
                .vsync
        );
        assert!(
            Session::open(root.path(), "other", "creator")
                .unwrap()
                .preferences
                .vsync
        );
        assert!(
            Session::open(root.path(), "game", "other")
                .unwrap()
                .preferences
                .vsync
        );
        let mut invalid = old;
        invalid["vsync"] = serde_json::json!("false");
        let bytes = serde_json::to_vec(&invalid).unwrap();
        std::fs::write(&session.path, &bytes).unwrap();
        assert!(Session::open(root.path(), "game", "creator").is_err());
        assert!(session.persist().is_ok()); // Unchanged session never overwrites a damaged record.
        session.preferences.vsync = true;
        assert!(session.persist().is_err());
        assert_eq!(std::fs::read(&session.path).unwrap(), bytes);
    }
    #[test]
    fn records_follow_game_creator_and_library_but_survive_version_changes() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut session = Session::open(a.path(), "game", "creator").unwrap();
        session
            .preferences
            .observe(Some([1200.0, 800.0]), Some(false), Some(false), false);
        session
            .preferences
            .observe(Some([1920.0, 1080.0]), Some(true), Some(false), false);
        session.persist().unwrap();
        let reloaded = Session::open(a.path(), "game", "creator").unwrap();
        assert_eq!(reloaded.preferences, session.preferences);
        assert_eq!(
            (reloaded.preferences.width, reloaded.preferences.height),
            (1200, 800)
        );
        for (root, id, creator) in [
            (a.path(), "other", "creator"),
            (a.path(), "game", "other"),
            (b.path(), "game", "creator"),
        ] {
            assert_eq!(
                Session::open(root, id, creator).unwrap().preferences,
                Preferences::default()
            );
        }
        let bytes = std::fs::read(&session.path).unwrap();
        std::fs::write(&session.path, b"damaged").unwrap();
        session.preferences.fullscreen = false;
        assert!(session.persist().is_err());
        assert_eq!(std::fs::read(&session.path).unwrap(), b"damaged");
        for bad in [
            b"{}".to_vec(),
            vec![b' '; 257],
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"version\":1", "\"version\":2")
                .into_bytes(),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"width\":1200", "\"width\":0")
                .into_bytes(),
            String::from_utf8(bytes)
                .unwrap()
                .replace("\"height\":800", "\"height\":10001")
                .into_bytes(),
        ] {
            std::fs::write(&session.path, bad).unwrap();
            assert!(Session::open(a.path(), "game", "creator").is_err());
        }
    }
    #[test]
    fn minimize_fullscreen_maximize_and_invalid_geometry_preserve_normal_bounds() {
        let mut p = Preferences::default();
        p.observe(Some([1100.5, 760.0]), Some(false), Some(false), false);
        assert_eq!((p.width, p.height), (1101, 760));
        for size in [
            [0.0, 0.0],
            [f32::NAN, 480.0],
            [f32::INFINITY, 480.0],
            [319.0, 480.0],
            [900.0, 10001.0],
        ] {
            p.observe(Some(size), None, None, false);
        }
        p.observe(Some([1.0, 1.0]), Some(false), Some(true), true);
        assert!(!p.maximized);
        p.observe(Some([1800.0, 1000.0]), Some(false), Some(true), false);
        p.observe(Some([1920.0, 1080.0]), Some(true), Some(false), false);
        assert!(p.maximized);
        p.observe(None, Some(false), Some(false), false);
        assert_eq!((p.width, p.height), (1101, 760));
        p.observe(Some([800.0, 600.0]), Some(false), Some(false), false);
        assert_eq!((p.width, p.height), (800, 600));
    }
    #[cfg(unix)]
    #[test]
    fn links_devices_and_replaced_paths_are_rejected_without_touching_targets() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        symlink(elsewhere.path(), root.path().join("window-preferences")).unwrap();
        assert!(Session::open(root.path(), "game", "creator").is_err());
        std::fs::remove_file(root.path().join("window-preferences")).unwrap();
        let mut session = Session::open(root.path(), "game", "creator").unwrap();
        session.persist().unwrap();
        std::fs::remove_file(&session.path).unwrap();
        let target = elsewhere.path().join("keep");
        std::fs::write(&target, b"keep").unwrap();
        symlink(&target, &session.path).unwrap();
        assert!(Session::open(root.path(), "game", "creator").is_err());
        session.preferences.width = 1000;
        assert!(session.persist().is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"keep");
        std::fs::remove_file(&session.path).unwrap();
        symlink("/dev/zero", &session.path).unwrap();
        assert!(Session::open(root.path(), "game", "creator").is_err());
    }
}
