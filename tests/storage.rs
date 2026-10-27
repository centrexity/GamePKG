use gamepkg::{manifest::Manifest, storage};
use std::fs;
#[test]
fn storage_counts_game_data_and_deduplicates_shared_references() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let mut m = Manifest::parse(
        &fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/minimal/manifest.toml"
        ))
        .unwrap(),
    )
    .unwrap();
    m.creator_key = hex::encode([4u8; 32]);
    fs::write(root.join(format!("{}.gamepkg", m.id)), b"package").unwrap();
    let hash = gamepkg::verify::digest(b"package");
    fs::create_dir_all(root.join("builds").join(&hash)).unwrap();
    fs::write(root.join("builds").join(hash).join("game.wasm"), b"build").unwrap();
    let cache = root.join("shader-builds/ours");
    fs::create_dir_all(&cache).unwrap();
    let metadata = toml::to_string(&m).unwrap();
    fs::write(cache.join("manifest.toml"), &metadata).unwrap();
    fs::write(cache.join("0.shader"), b"shader").unwrap();
    let other = root.join("shader-builds/other");
    fs::create_dir_all(&other).unwrap();
    let mut other_game = m.clone();
    other_game.creator_key = hex::encode([5u8; 32]);
    fs::write(
        other.join("manifest.toml"),
        toml::to_string(&other_game).unwrap(),
    )
    .unwrap();
    fs::write(other.join("0.shader"), b"unrelated").unwrap();
    fs::create_dir(root.join("saves")).unwrap();
    let save =
        gamepkg::verify::digest(format!("GamePKG save v1\0{}\0{}", m.id, m.creator_key).as_bytes());
    fs::write(root.join("saves").join(format!("{save}.blob")), b"save").unwrap();
    fs::create_dir(root.join("progress")).unwrap();
    let history = gamepkg::verify::digest(
        format!("GamePKG progress v1\0{}\0{}", m.id, m.creator_key).as_bytes(),
    );
    fs::write(
        root.join("progress").join(format!("{history}.json")),
        b"history",
    )
    .unwrap();
    fs::create_dir(root.join("asset-pool")).unwrap();
    let shared = gamepkg::pool::Reference {
        sha256: gamepkg::verify::digest(b"shared"),
        size: 6,
    };
    fs::write(
        root.join("asset-pool")
            .join(format!("{}.blob", shared.sha256)),
        b"shared",
    )
    .unwrap();
    m.shared_assets.insert("one.bin".into(), shared.clone());
    m.shared_assets.insert("two.bin".into(), shared);
    let mut windows =
        gamepkg::window_preferences::Session::open(root, &m.id, &m.creator_key).unwrap();
    windows.persist().unwrap();
    let window_bytes = fs::metadata(
        fs::read_dir(root.join("window-preferences"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap()
    .len();
    let usage = storage::usage(root, &m).unwrap();
    assert_eq!(usage.package_bytes, 7);
    assert_eq!(usage.built_bytes, 5);
    assert_eq!(usage.shader_bytes, metadata.len() as u64 + 6);
    assert_eq!(usage.save_bytes, 4);
    assert_eq!(usage.history_bytes, 7);
    assert_eq!(usage.window_bytes, window_bytes);
    assert_eq!(usage.shared_bytes, 6);
    assert_eq!(
        usage.used_bytes(),
        7 + 5 + metadata.len() as u64 + 6 + 4 + 7 + 6 + window_bytes
    );
    assert!(storage::total(root).unwrap() > usage.used_bytes());
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join(format!("{save}.blob")), vec![1; 8192]).unwrap();
        fs::rename(root.join("saves"), root.join("old-saves")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("saves")).unwrap();
        assert_eq!(storage::usage(root, &m).unwrap().save_bytes, 0);
    }
}
#[cfg(unix)]
#[test]
fn storage_inventory_does_not_follow_links_outside_the_library() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("large"), vec![1; 4096]).unwrap();
    fs::write(root.path().join("owned"), b"ours").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("external")).unwrap();
    assert_eq!(storage::total(root.path()).unwrap(), 4);
}

#[test]
fn reviewed_native_storage_counts_both_platform_caches_without_charging_other_games() {
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    let mut manifest = Manifest::parse(include_bytes!("../ports/takedawn/manifest.toml")).unwrap();
    manifest.creator_key = hex::encode([4u8; 32]);
    fs::write(root.join(format!("{}.gamepkg", manifest.id)), b"package").unwrap();
    let windows = root.join("native-unit-cache/takedawn-windows");
    fs::create_dir_all(&windows).unwrap();
    fs::write(windows.join("unit.o"), b"windows-obj").unwrap();
    fs::write(windows.join("unit.sha256"), b"checkpoint").unwrap();
    // A native Windows library has no Linux cache. Its compiled work must still
    // appear on the game page, even though the cache lives outside builds/.
    assert_eq!(storage::usage(root, &manifest).unwrap().built_bytes, 21);

    let linux = root.join("native-unit-cache/takedawn");
    fs::create_dir_all(&linux).unwrap();
    fs::write(linux.join("unit.o"), b"linux-obj").unwrap();
    let build = root.join("builds").join(gamepkg::verify::digest(b"package"));
    fs::create_dir_all(&build).unwrap();
    fs::write(build.join("native-game"), b"native").unwrap();
    for folder in ["native-sdk/tools", "native-unit-cache/another-engine"] {
        let folder = root.join(folder);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("unrelated"), vec![0; 4096]).unwrap();
    }
    assert_eq!(storage::usage(root, &manifest).unwrap().built_bytes, 36);
    let mut other = Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap();
    other.creator_key = hex::encode([5u8; 32]);
    fs::write(root.join(format!("{}.gamepkg", other.id)), b"other package").unwrap();
    assert_eq!(storage::usage(root, &other).unwrap().built_bytes, 0);

    #[cfg(unix)]
    {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("external"), vec![0; 8192]).unwrap();
        fs::rename(&windows, root.join("old-windows-cache")).unwrap();
        std::os::unix::fs::symlink(outside.path(), windows).unwrap();
        assert_eq!(storage::usage(root, &manifest).unwrap().built_bytes, 15);
    }
}
