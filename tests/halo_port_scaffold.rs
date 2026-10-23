use std::path::Path;

fn manifest_text() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo/manifest.toml"))
        .unwrap()
}

fn engine() -> &'static gamepkg::external_engine::Engine {
    gamepkg::external_engine::get("opence-ce95f7de").unwrap()
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap()
}

#[test]
fn external_engine_recipe_parses_and_rejects_layout_and_network_changes() {
    let manifest = gamepkg::manifest::Manifest::parse(manifest_text().as_bytes()).unwrap();
    assert_eq!(
        manifest.builtin_engine.as_deref(),
        Some(engine().id.as_str())
    );
    assert_eq!(
        manifest.game_data.as_ref().unwrap().layouts,
        engine().layouts()
    );
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
    assert_eq!(pin["commit"].as_str(), Some(engine().commit.as_str()));
    assert_eq!(
        pin["license_sha256"].as_str(),
        Some(engine().license_sha256.as_str())
    );
    assert_eq!(pin["vendored"], false);
    for (needle, message) in [
        ("network = false", "network = true"),
        (
            "hard_memory_bytes = 3221225472",
            "hard_memory_bytes = 536870912",
        ),
        ("directories = [\"maps\"]", "directories = [\"other\"]"),
    ] {
        let error =
            gamepkg::manifest::Manifest::parse(manifest_text().replace(needle, message).as_bytes())
                .unwrap_err()
                .to_string();
        assert!(
            error.contains("external engine") || error.contains("invalid resource limits"),
            "{needle} -> {error}"
        );
    }
    let text = manifest_text();
    let start = text.find("[game_data]").unwrap();
    let end = text.find("[limits]").unwrap();
    let removed = format!("{}{}", &text[..start], &text[end..]);
    let error = gamepkg::manifest::Manifest::parse(removed.as_bytes())
        .unwrap_err()
        .to_string();
    assert!(error.contains("requires a folder"), "{error}");
}

#[test]
fn external_engine_package_contains_no_disc_or_map_and_only_the_launcher() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo");
    let launcher = std::fs::read(root.join("source/src/lib.rs")).unwrap();
    assert_eq!(launcher, engine().launcher.as_bytes());
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
fn data_layouts_accept_maps_or_one_archive() {
    let layouts = &gamepkg::manifest::Manifest::parse(manifest_text().as_bytes())
        .unwrap()
        .game_data
        .unwrap()
        .layouts;
    let root = tempfile::tempdir().unwrap();
    let missing = root.path().join("empty");
    std::fs::create_dir(&missing).unwrap();
    assert!(gamepkg::game_data::matches(&missing, layouts).is_err());
    let disc = root.path().join("disc");
    std::fs::create_dir(&disc).unwrap();
    std::fs::write(disc.join("halo.iso"), vec![0; 2048]).unwrap();
    match gamepkg::game_data::matches(&disc, layouts).unwrap() {
        gamepkg::game_data::DataMatch::Beside { file } => {
            assert!(file.ends_with("halo.iso"));
        }
        other => panic!("expected an archive, got {other:?}"),
    }
    std::fs::write(disc.join("second.xiso"), vec![0; 2048]).unwrap();
    assert!(gamepkg::game_data::matches(&disc, layouts).is_err());
    let maps = root.path().join("maps-root");
    std::fs::create_dir_all(maps.join("maps")).unwrap();
    std::fs::write(maps.join("maps/ui.map"), b"not-a-real-map").unwrap();
    assert!(matches!(
        gamepkg::game_data::matches(&maps, layouts).unwrap(),
        gamepkg::game_data::DataMatch::Link { .. }
    ));
}

#[test]
fn blocked_targets_fail_before_fetching_source() {
    let library = tempfile::tempdir().unwrap();
    let package = library.path().join("halo.gamepkg");
    gamepkg::pack::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo"),
        &package,
        &[9; 32],
    )
    .unwrap();
    gamepkg::registry::install(library.path(), &package, true).unwrap();
    for target in ["linux-x86_64", "windows-x64"] {
        let error = gamepkg::native::compile_for(library.path(), "com.gamepkg.halo", Some(target))
            .unwrap_err()
            .to_string();
        assert!(error.contains("32-bit pointers"), "{target}: {error}");
    }
    assert!(library.path().join("engine-sources").read_dir().is_err());
}

#[cfg(target_os = "linux")]
#[test]
fn cross_built_windows_i686_engine_caches_and_refuses_to_launch() {
    let _lock = env_lock();
    let source = std::path::PathBuf::from("/tmp/OpenCE-win");
    if !source.join("build/windows/halo.exe").is_file() {
        eprintln!(
            "skipping Windows cache test; no cross build at {}",
            source.display()
        );
        return;
    }
    let previous = std::env::var_os("GAMEPKG_ENGINE_SOURCE");
    std::env::set_var("GAMEPKG_ENGINE_SOURCE", &source);
    let library = tempfile::tempdir().unwrap();
    let package = library.path().join("halo.gamepkg");
    gamepkg::pack::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo"),
        &package,
        &[9; 32],
    )
    .unwrap();
    gamepkg::registry::install(library.path(), &package, true).unwrap();
    gamepkg::native::compile_for(library.path(), "com.gamepkg.halo", Some("windows-i686")).unwrap();
    assert!(gamepkg::native::is_built(library.path(), "com.gamepkg.halo").unwrap());
    let error = gamepkg::external_engine::launch(library.path(), "com.gamepkg.halo")
        .unwrap_err()
        .to_string();
    match previous {
        Some(source) => std::env::set_var("GAMEPKG_ENGINE_SOURCE", source),
        None => std::env::remove_var("GAMEPKG_ENGINE_SOURCE"),
    }
    assert!(
        error.contains("windows") && error.contains("built for"),
        "{error}"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn pinned_checkout_caches_the_i686_engine_without_copying_game_data() {
    let _lock = env_lock();
    let source = std::env::var_os("GAMEPKG_ENGINE_SOURCE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/OpenCE"));
    if !source.join("build/linux/halo").is_file() {
        eprintln!(
            "skipping external engine cache test; no local build at {}",
            source.display()
        );
        return;
    }
    let previous = std::env::var_os("GAMEPKG_ENGINE_SOURCE");
    std::env::set_var("GAMEPKG_ENGINE_SOURCE", &source);
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
    let error = gamepkg::external_engine::launch(library.path(), "com.gamepkg.halo")
        .unwrap_err()
        .to_string();
    match previous_home {
        Some(home) => std::env::set_var("GAMEPKG_HOME", home),
        None => std::env::remove_var("GAMEPKG_HOME"),
    }
    match previous {
        Some(source) => std::env::set_var("GAMEPKG_ENGINE_SOURCE", source),
        None => std::env::remove_var("GAMEPKG_ENGINE_SOURCE"),
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
