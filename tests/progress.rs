use gamepkg::{archive::Package, pack::pack, registry, runtime::Game, verify::verify_embedded};
use std::{fs, path::Path};
fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for item in fs::read_dir(from).unwrap() {
        let item = item.unwrap();
        if item.file_type().unwrap().is_dir() {
            copy(&item.path(), &to.join(item.file_name()));
        } else {
            fs::copy(item.path(), to.join(item.file_name())).unwrap();
        }
    }
}
#[test]
fn play_history_achievements_and_storage_survive_sessions() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    copy(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &source,
    );
    let file = source.join("manifest.toml");
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file,format!("{text}\n[[achievements]]\nid='first_step'\ntitle='First step'\ndescription='Start playing'\n")).unwrap();
    let package = temp.path().join("game.gamepkg");
    pack(&source, &package, &[4; 32]).unwrap();
    let manifest = verify_embedded(&mut Package::open(&package).unwrap()).unwrap();
    let root = temp.path().join("library");
    registry::install(&root, &package, true).unwrap();
    let cache = gamepkg::build::cache_path(&root, &manifest.id).unwrap();
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    let good=wat::parse_str("(module (import \"gamepkg\" \"achievement_unlock\" (func $unlock (param i32) (result i32))) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i32.const -1 call $unlock drop i32.const 0 call $unlock drop i64.const 0))").unwrap();
    fs::write(&cache, &good).unwrap();
    gamepkg::integrity::record_build(&cache, &good).unwrap();
    fs::write(root.join("unrelated-broken.gamepkg"), b"damaged").unwrap();
    gamepkg::integrity::check(&root, &manifest.id).unwrap();
    {
        let mut game = Game::load(&root, &manifest.id).unwrap();
        game.step(0).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        game.step(0).unwrap();
        game.finish().unwrap();
    }
    let record = gamepkg::progress::load(&root, &manifest.id, &manifest.creator_key).unwrap();
    assert!(record.played_ms >= 20);
    assert_eq!(record.unlocked.len(), 1);
    let timestamp = record.unlocked["first_step"];
    {
        let mut game = Game::load(&root, &manifest.id).unwrap();
        game.step(0).unwrap();
        game.finish().unwrap();
    }
    assert_eq!(
        gamepkg::progress::load(&root, &manifest.id, &manifest.creator_key)
            .unwrap()
            .unlocked["first_step"],
        timestamp
    );
    assert_eq!(
        gamepkg::progress::load(&root, &manifest.id, "other-creator")
            .unwrap()
            .played_ms,
        0
    );
    let usage = gamepkg::storage::usage(&root, &manifest).unwrap();
    assert_eq!(
        usage.package_bytes,
        fs::metadata(root.join(format!("{}.gamepkg", manifest.id)))
            .unwrap()
            .len()
    );
    assert_eq!(usage.built_bytes, good.len() as u64 + 64);
    assert!(usage.estimated_bytes.is_none());
    let mut estimated = manifest.clone();
    estimated.limits.recommended_storage_bytes = Some(1);
    assert_eq!(
        gamepkg::storage::usage(&root, &estimated)
            .unwrap()
            .estimated_bytes,
        Some(usage.package_bytes)
    );
    assert!(gamepkg::storage::total(&root).unwrap() >= usage.package_bytes + usage.built_bytes);
    let history_path = fs::read_dir(root.join("progress"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|x| x == "json"))
        .unwrap();
    let history_bytes = fs::read(&history_path).unwrap();
    fs::write(&history_path, b"corrupt history").unwrap();
    let listed = std::process::Command::new(env!("CARGO_BIN_EXE_gamepkg"))
        .env("GAMEPKG_HOME", &root)
        .arg("list")
        .output()
        .unwrap();
    assert!(listed.status.success());
    let text = String::from_utf8(listed.stdout).unwrap();
    assert!(text.contains("unavailable") && text.contains(&manifest.id));
    fs::write(&history_path, history_bytes).unwrap();
    // A trap must not unlock anything, even if the guest called the API first.
    let other_root = temp.path().join("trapped");
    registry::install(&other_root, &package, true).unwrap();
    let other_cache = gamepkg::build::cache_path(&other_root, &manifest.id).unwrap();
    fs::create_dir_all(other_cache.parent().unwrap()).unwrap();
    fs::write(&other_cache,wat::parse_str("(module (import \"gamepkg\" \"achievement_unlock\" (func $u (param i32) (result i32))) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i32.const 0 call $u drop unreachable))").unwrap()).unwrap();
    gamepkg::integrity::record_build(&other_cache, &fs::read(&other_cache).unwrap()).unwrap();
    {
        let mut game = Game::load(&other_root, &manifest.id).unwrap();
        assert!(game.step(0).is_err());
    }
    assert!(
        gamepkg::progress::load(&other_root, &manifest.id, &manifest.creator_key)
            .unwrap()
            .unlocked
            .is_empty()
    );
}
#[test]
fn achievement_manifest_rejects_duplicates_and_invalid_identifiers() {
    let text = fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap();
    for suffix in ["[[achievements]]\nid='../escape'\ntitle='x'\ndescription='x'", "[[achievements]]\nid='same'\ntitle='x'\ndescription='x'\n[[achievements]]\nid='same'\ntitle='y'\ndescription='y'"] { assert!(gamepkg::manifest::Manifest::parse(format!("{text}\n{suffix}").as_bytes()).is_err()); }
}
