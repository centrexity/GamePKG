//! Real signed-package state/persistence checks; no player library is touched.
use gamepkg::runtime::Game;
use std::path::{Path, PathBuf};
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        if name == "saves" || name == "progress" {
            continue;
        }
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &to.join(name));
        } else {
            std::fs::copy(entry.path(), to.join(name)).unwrap();
        }
    }
}
fn save_path(root: &Path) -> PathBuf {
    let files: Vec<_> = std::fs::read_dir(root.join("saves"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    files[0].clone()
}
fn number(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn initial_save() -> Vec<u8> {
    let mut bytes = b"GP48\x01\r\n\x1a".to_vec();
    bytes.extend(0u64.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(1u32.to_le_bytes());
    bytes.extend([1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    bytes.extend(0u64.to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes
}
#[test]
#[ignore = "requires GAMEPKG_2048_TEST_ROOT with fresh native and Wasm builds"]
fn actual_engines_share_scoring_undo_and_saved_boards_without_held_repeats() {
    let source = PathBuf::from(std::env::var_os("GAMEPKG_2048_TEST_ROOT").unwrap());
    for native in [true, false] {
        let root = tempfile::tempdir().unwrap();
        copy(&source, root.path());
        let load =
            |native| Game::load_target(root.path(), "com.gamepkg.2048", &[], Some(native)).unwrap();
        let mut game = load(native);
        game.step(0).unwrap();
        game.finish().unwrap();
        drop(game);
        let path = save_path(root.path());
        std::fs::write(&path, initial_save()).unwrap();
        let mut game = load(native);
        assert_eq!(game.step(1).unwrap() >> 32, 4);
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(&saved[..8], b"GP48\x01\r\n\x1a");
        assert_eq!(number(&saved, 8), 4);
        assert_eq!(number(&saved, 40), 4);
        assert_eq!(saved[24], 2);
        assert_eq!(u32::from_le_bytes(saved[16..20].try_into().unwrap()), 1);
        for _ in 0..30 {
            game.step(1 << 8).unwrap();
        }
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        game.finish().unwrap();
        drop(game);
        // Both targets use the same fixed-width save, including undo history.
        let mut game = load(!native);
        assert_eq!(game.step(0).unwrap() >> 32, 4);
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        assert_eq!(game.step(16).unwrap() >> 32, 0);
        let undone = std::fs::read(&path).unwrap();
        assert_eq!(&undone[24..40], &initial_save()[24..40]);
        assert_eq!(number(&undone, 8), 4);
        game.step(64).unwrap();
        let restarted = std::fs::read(&path).unwrap();
        assert_eq!(number(&restarted, 40), 0);
        assert_eq!(number(&restarted, 8), 4);
        assert_eq!(
            restarted[24..40].iter().filter(|cell| **cell != 0).count(),
            2
        );
        game.finish().unwrap();
    }
}
#[test]
#[ignore = "requires GAMEPKG_2048_TEST_ROOT with fresh native and Wasm builds"]
fn damaged_saves_are_preserved_until_the_player_explicitly_starts_again() {
    let source = PathBuf::from(std::env::var_os("GAMEPKG_2048_TEST_ROOT").unwrap());
    for native in [true, false] {
        let root = tempfile::tempdir().unwrap();
        copy(&source, root.path());
        let load =
            || Game::load_target(root.path(), "com.gamepkg.2048", &[], Some(native)).unwrap();
        let mut game = load();
        game.step(0).unwrap();
        game.finish().unwrap();
        drop(game);
        let path = save_path(root.path());
        let broken = b"GP48\x7f\r\n\x1aunknown version";
        std::fs::write(&path, broken).unwrap();
        let mut game = load();
        for input in [0, 1, 2, 4, 8, 16, 65536] {
            game.step(input).unwrap();
        }
        assert_eq!(std::fs::read(&path).unwrap(), broken);
        assert!(!game.frame().commands.is_empty());
        game.step(64).unwrap();
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(&saved[..8], b"GP48\x01\r\n\x1a");
        assert_eq!(saved.len(), 52);
        game.finish().unwrap();
    }
}

#[test]
#[ignore = "requires GAMEPKG_2048_TEST_ROOT with the denied-save fixture"]
fn denied_saving_does_not_create_a_blob_or_prevent_gameplay() {
    let source = PathBuf::from(std::env::var_os("GAMEPKG_2048_TEST_ROOT").unwrap());
    for native in [true, false] {
        let root = tempfile::tempdir().unwrap();
        copy(&source, root.path());
        let mut game =
            Game::load_target(root.path(), "com.gamepkg.2048-nosave", &[], Some(native)).unwrap();
        let initial = game.step(0).unwrap();
        let mut changed = false;
        for input in [1, 4, 2, 8, 1, 4, 2, 8] {
            changed |= game.step(input).unwrap() != initial;
        }
        assert!(changed);
        assert!(!game.frame().commands.is_empty());
        game.finish().unwrap();
        assert!(
            !root.path().join("saves").exists()
                || std::fs::read_dir(root.path().join("saves"))
                    .unwrap()
                    .next()
                    .is_none()
        );
    }
}
