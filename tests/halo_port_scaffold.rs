use std::path::Path;

fn manifest_text() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo/manifest.toml"))
        .unwrap()
}

#[test]
fn opence_recipe_parses_and_rejects_game_data_and_network_changes() {
    let manifest = gamepkg::manifest::Manifest::parse(manifest_text().as_bytes()).unwrap();
    assert_eq!(
        manifest.builtin_engine.as_deref(),
        Some(gamepkg::opence::PROFILE)
    );
    assert!(manifest.game_data.is_some());
    assert!(!manifest.permissions.network);
    let catalog = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/ports.gamepkglist"),
    )
    .unwrap();
    assert!(!catalog.contains("com.gamepkg.halo"));
    let pin: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/tools/opence-pin.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(pin["commit"].as_str(), Some(gamepkg::opence::COMMIT));
    assert_eq!(
        pin["license_sha256"].as_str(),
        Some(gamepkg::opence::LICENSE_SHA256)
    );
    assert_eq!(pin["vendored"], false);
    for (needle, message) in [
        ("network = false", "network = true"),
        (
            "hard_memory_bytes = 3221225472",
            "hard_memory_bytes = 536870912",
        ),
    ] {
        let error =
            gamepkg::manifest::Manifest::parse(manifest_text().replace(needle, message).as_bytes())
                .unwrap_err()
                .to_string();
        assert!(
            error.contains("OpenCE") || error.contains("invalid resource limits"),
            "{needle} -> {error}"
        );
    }
    let removed = manifest_text().replace(
        "[game_data]\ntitle = \"Folder with your Halo maps directory or one Xbox disc image\"\n\n",
        "",
    );
    let error = gamepkg::manifest::Manifest::parse(removed.as_bytes())
        .unwrap_err()
        .to_string();
    assert!(error.contains("OpenCE requires a folder"), "{error}");
}

#[test]
fn opence_package_contains_no_disc_or_map_and_only_the_launcher() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo");
    let launcher = std::fs::read(root.join("source/src/lib.rs")).unwrap();
    assert_eq!(launcher, gamepkg::opence::LAUNCHER.as_bytes());
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("halo.gamepkg");
    gamepkg::pack::pack(&root, &package, &[9; 32]).unwrap();
    let mut archive = gamepkg::archive::Package::open(&package).unwrap();
    let manifest = gamepkg::verify::verify(
        &mut archive,
        ed25519_dalek::SigningKey::from_bytes(&[9; 32])
            .verifying_key()
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(manifest.id, "com.gamepkg.halo");
    for name in archive.entries.keys() {
        let lower = name.to_ascii_lowercase();
        assert!(
            !lower.ends_with(".iso")
                && !lower.ends_with(".xiso")
                && !lower.ends_with(".map")
                && !lower.contains("maps"),
            "{name}"
        );
    }
}

#[test]
fn halo_data_folder_must_be_maps_or_one_disc_image() {
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("empty");
    std::fs::create_dir(&missing).unwrap();
    assert!(gamepkg::opence::classify(&missing).is_err());
    let disc = root.path().join("disc");
    std::fs::create_dir(&disc).unwrap();
    std::fs::write(disc.join("halo.iso"), vec![0; 2048]).unwrap();
    match gamepkg::opence::classify(&disc).unwrap() {
        gamepkg::opence::GameData::Disc(_, image) => {
            assert!(image.ends_with("halo.iso"));
        }
        other => panic!("expected a disc, got {other:?}"),
    }
    std::fs::write(disc.join("second.xiso"), vec![0; 2048]).unwrap();
    assert!(gamepkg::opence::classify(&disc).is_err());
    let maps = root.path().join("maps-root");
    std::fs::create_dir_all(maps.join("maps")).unwrap();
    std::fs::write(maps.join("maps/ui.map"), b"not-a-real-map").unwrap();
    assert!(matches!(
        gamepkg::opence::classify(&maps).unwrap(),
        gamepkg::opence::GameData::Maps(_)
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn pinned_checkout_caches_the_i686_engine_without_copying_game_data() {
    let source = std::env::var_os("GAMEPKG_OPENCE_SOURCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/OpenCE"));
    if !source.join("build/linux/halo").is_file() {
        eprintln!(
            "skipping OpenCE cache test; no local build at {}",
            source.display()
        );
        return;
    }
    std::env::set_var("GAMEPKG_OPENCE_SOURCE", &source);
    let library = tempfile::tempdir().unwrap();
    let package = library.path().join("halo.gamepkg");
    gamepkg::pack::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo"),
        &package,
        &[9; 32],
    )
    .unwrap();
    gamepkg::registry::install(library.path(), &package, true).unwrap();
    gamepkg::native::compile(library.path(), "com.gamepkg.halo").unwrap();
    assert!(gamepkg::native::is_built(library.path(), "com.gamepkg.halo").unwrap());
    let report = gamepkg::integrity::check(library.path(), "com.gamepkg.halo").unwrap();
    assert!(report.contains("compiled game verified"), "{report}");
    let data = library.path().join("not-real");
    std::fs::create_dir_all(data.join("maps")).unwrap();
    std::fs::write(data.join("maps/ui.map"), b"placeholder").unwrap();
    gamepkg::game_data::remember_for_game(library.path(), "com.gamepkg.halo", &data).unwrap();
    let previous_home = std::env::var_os("GAMEPKG_HOME");
    std::env::set_var("GAMEPKG_HOME", library.path());
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    std::env::set_var("SDL_AUDIODRIVER", "dummy");
    std::env::set_var("HALO_HIDDEN_WINDOW", "1");
    std::env::set_var("HALO_NULL_RENDERER", "1");
    let error = gamepkg::opence::launch(library.path(), "com.gamepkg.halo")
        .unwrap_err()
        .to_string();
    if let Some(home) = previous_home {
        std::env::set_var("GAMEPKG_HOME", home);
    }
    assert!(
        error.contains("ui.map") || error.contains("Xbox maps") || error.contains("not the Xbox"),
        "{error}"
    );
    let runtime = std::fs::read_dir(library.path().join("runtime")).unwrap();
    for entry in runtime {
        let path = entry.unwrap().path();
        assert!(
            !path.join("maps").join("ui.map").exists(),
            "placeholder map must not be treated as installed game data"
        );
    }
}
