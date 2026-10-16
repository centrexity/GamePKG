use super::{App, Work};
use std::path::PathBuf;

fn dropped_path(files: &[eframe::egui::DroppedFile]) -> Result<PathBuf, &'static str> {
    if files.len() != 1 {
        return Err("Drop one .gamepkg or .gamepkglist file at a time.");
    }
    let path = files[0]
        .path
        .clone()
        .ok_or("Drop a local file from your file manager.")?;
    if !path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("gamepkg") || e.eq_ignore_ascii_case("gamepkglist"))
    {
        return Err("Supported files: .gamepkg packages and .gamepkglist catalogs.");
    }
    Ok(path)
}
fn import_catalog(root: &std::path::Path, path: &std::path::Path) -> anyhow::Result<usize> {
    let bytes = crate::files::read_regular(path, 4 * 1024 * 1024, true)?;
    let snapshot = tempfile::NamedTempFile::new()?;
    std::fs::write(snapshot.path(), bytes)?;
    crate::catalog::import(root, snapshot.path())
}
impl App {
    pub(super) fn handle_drops(&mut self, ctx: &eframe::egui::Context) {
        let files = ctx.input(|i| i.raw.dropped_files.clone());
        if files.is_empty() {
            return;
        }
        let outcome = (|| {
            if self.player.is_some() {
                return Err("Return to the library before dropping a file.");
            }
            if self.result.is_some() {
                return Err("Wait for the current operation to finish, then drop your file again.");
            }
            if self.pending.is_some() || self.pending_store.is_some() {
                return Err(
                    "Finish or cancel the current package review before dropping another file.",
                );
            }
            dropped_path(&files)
        })();
        let path = match outcome {
            Ok(path) => path,
            Err(message) => {
                self.drop_notice = Some(message.into());
                return;
            }
        };
        self.drop_notice = None;
        self.show_settings = false;
        self.show_support = false;
        self.preview = None;
        self.source = path.to_string_lossy().into_owned();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("gamepkglist"))
        {
            self.manifest = None;
            self.begin_work(move || {
                let count = import_catalog(&crate::registry::default_root()?, &path)?;
                Ok(Work::Message(format!(
                    "Added {count} games to the catalog. No game packages downloaded."
                )))
            });
        } else {
            self.inspect_package(self.source.clone(), Some(path));
        }
        // Drops can arrive on an idle window; start polling the background worker.
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(path: impl Into<PathBuf>) -> eframe::egui::DroppedFile {
        eframe::egui::DroppedFile {
            path: Some(path.into()),
            ..Default::default()
        }
    }
    fn drop(app: &mut App, files: Vec<eframe::egui::DroppedFile>) {
        let ctx = eframe::egui::Context::default();
        let _ = ctx.run(
            eframe::egui::RawInput {
                dropped_files: files,
                ..Default::default()
            },
            |ctx| app.handle_drops(ctx),
        );
    }
    #[test]
    fn native_drop_checks_type_cardinality_and_preserves_pending_work() {
        assert!(dropped_path(&[file("GAME.GAMEPKG")]).is_ok());
        assert!(dropped_path(&[file("list.gamepkglist")]).is_ok());
        for files in [
            vec![],
            vec![file("a.gamepkg"), file("b.gamepkglist")],
            vec![file("game.exe")],
            vec![file("game.gamepkg.exe")],
            vec![eframe::egui::DroppedFile::default()],
        ] {
            assert!(dropped_path(&files).is_err());
        }
        let mut app = App {
            pending: Some(tempfile::NamedTempFile::new().unwrap()),
            ..Default::default()
        };
        let before = app.pending.as_ref().unwrap().path().to_owned();
        drop(&mut app, vec![file("other.gamepkg")]);
        assert_eq!(app.pending.as_ref().unwrap().path(), before);
        assert!(app.result.is_none());
        assert!(app.drop_notice.is_some());
        let (_tx, rx) = std::sync::mpsc::channel();
        app.pending = None;
        app.result = Some(rx);
        drop(&mut app, vec![file("other.gamepkglist")]);
        assert!(app.result.is_some());
    }
    #[test]
    fn dropped_package_is_verified_in_a_private_snapshot_and_never_installed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("sample with spaces.gamepkg");
        crate::pack::pack(
            std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/minimal")),
            &path,
            &[61; 32],
        )
        .unwrap();
        let mut app = App::default();
        drop(&mut app, vec![file(&path)]);
        let stage = app.pending.as_ref().unwrap().path().to_owned();
        assert_ne!(stage, path);
        let result = app
            .result
            .take()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
            .unwrap();
        let Work::Package(m) = result else {
            panic!("expected package review");
        };
        assert_eq!(m.id, "com.gamepkg.minimal");
        std::fs::write(&path, b"changed after review").unwrap();
        assert!(crate::verify::verify_embedded(
            &mut crate::archive::Package::open(&stage).unwrap()
        )
        .is_ok());
    }
    #[test]
    fn dropped_catalog_adds_entries_without_download_or_install() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("library");
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/lists/starter.gamepkglist"
        ));
        assert!(import_catalog(&root, path).unwrap() > 0);
        assert!(!crate::catalog::entries(&root).unwrap().is_empty());
        assert!(crate::registry::list(&root).unwrap().is_empty());
        assert!(!root.join("builds").exists());
        let bad = temp.path().join("bad.gamepkglist");
        std::fs::write(&bad, b"invalid").unwrap();
        let before = crate::catalog::entries(&root).unwrap();
        assert!(import_catalog(&root, &bad).is_err());
        assert_eq!(crate::catalog::entries(&root).unwrap(), before);
    }
    #[test]
    fn corrupt_drop_fails_verification() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("bad.gamepkg");
        std::fs::write(&path, b"not a package").unwrap();
        let mut app = App::default();
        drop(&mut app, vec![file(path)]);
        assert!(app
            .result
            .take()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
            .is_err());
    }
}
