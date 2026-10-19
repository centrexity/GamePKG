use super::App;
use crate::manifest::Manifest;
use anyhow::{ensure, Context, Result};
use std::path::PathBuf;

// Bind a dialog result to the library, creator and named slot it was opened for.
pub(super) struct Selection {
    root: PathBuf,
    manifest: Manifest,
    index: usize,
    path: Option<PathBuf>,
}
impl Selection {
    pub(super) fn new(root: PathBuf, manifest: Manifest, index: usize) -> Result<Self> {
        ensure!(index < manifest.required_rom_count(), "Unknown ROM slot");
        Ok(Self {
            root: std::fs::canonicalize(root)?,
            manifest,
            index,
            path: None,
        })
    }
    pub(super) fn with_path(mut self, path: Option<PathBuf>) -> Self {
        self.path = path;
        self
    }
    fn save(&self, current: &Manifest) -> Result<Option<Vec<String>>> {
        let Some(path) = &self.path else {
            return Ok(None);
        };
        ensure!(
            crate::rom_selections::identity(current)?
                == crate::rom_selections::identity(&self.manifest)?,
            "The game or ROM requirements changed. Choose the file again."
        );
        ensure!(
            path.is_file(),
            "The selected ROM file is no longer available"
        );
        let path = path
            .to_str()
            .context("This file location cannot be saved as text")?;
        let mut paths = crate::rom_selections::load(&self.root, current)?;
        paths[self.index] = path.to_owned();
        crate::rom_selections::save(&self.root, current, &paths)?;
        Ok(Some(paths))
    }
}
impl App {
    pub(super) fn apply_rom_pick(&mut self, selection: Selection) -> Result<bool> {
        if selection.path.is_none() {
            return Ok(false);
        }
        ensure!(
            std::fs::canonicalize(crate::registry::default_root()?)? == selection.root,
            "The library changed. Choose the file again."
        );
        let _change = crate::registry::LibraryChange::acquire(&selection.root)?;
        let current = crate::registry::installed_manifest(&selection.root, &selection.manifest.id)?;
        if let Some(paths) = selection.save(&current)? {
            self.rom_paths.insert(current.id.clone(), paths);
            self.rom_identity = None;
            self.sync_rom_selection(&selection.root);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> Manifest {
        let mut m: Manifest =
            toml::from_str(include_str!("../../examples/minimal/manifest.toml")).unwrap();
        m.requires_rom = true;
        m.roms = vec![
            crate::manifest::RomRequirement {
                id: "first".into(),
                title: "First".into(),
            },
            crate::manifest::RomRequirement {
                id: "second".into(),
                title: "Second".into(),
            },
        ];
        m
    }
    #[test]
    fn cancel_keeps_paths_and_selection_changes_only_its_named_slot() {
        let root = tempfile::tempdir().unwrap();
        let m = manifest();
        let original = vec![
            root.path().join("first.rom").to_str().unwrap().into(),
            root.path().join("second.rom").to_str().unwrap().into(),
        ];
        crate::rom_selections::save(root.path(), &m, &original).unwrap();
        let selection = Selection::new(root.path().into(), m.clone(), 1).unwrap();
        assert!(selection.save(&m).unwrap().is_none());
        assert_eq!(
            crate::rom_selections::load(root.path(), &m).unwrap(),
            original
        );
        let path = root.path().join("a ROM with spaces.rom");
        std::fs::write(&path, b"ROM").unwrap();
        let selection = selection.with_path(Some(path.clone()));
        let paths = selection.save(&m).unwrap().unwrap();
        assert_eq!(
            paths,
            vec![original[0].clone(), path.to_str().unwrap().into()]
        );
        assert_eq!(std::fs::read(path).unwrap(), b"ROM");
    }
    #[test]
    fn changed_creator_requirements_or_missing_file_do_not_change_remembered_paths() {
        let root = tempfile::tempdir().unwrap();
        let m = manifest();
        let path = root.path().join("rom");
        std::fs::write(&path, b"ROM").unwrap();
        let selection = Selection::new(root.path().into(), m.clone(), 0)
            .unwrap()
            .with_path(Some(path.clone()));
        let mut changed = m.clone();
        changed.creator_key = "different".into();
        assert!(selection.save(&changed).is_err());
        changed = m.clone();
        changed.roms.swap(0, 1);
        assert!(selection.save(&changed).is_err());
        std::fs::remove_file(path).unwrap();
        assert!(selection.save(&m).is_err());
        assert!(!root.path().join("rom-selections.json").exists());
        assert!(Selection::new(root.path().into(), m, 2).is_err());
    }
}
