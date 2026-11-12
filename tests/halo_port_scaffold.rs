use std::path::Path;

fn manifest_text() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/halo/manifest.toml"))
        .unwrap()
}

fn engine() -> &'static gamepkg::external_engine::Engine {
    gamepkg::external_engine::get("opence-ce95f7de").unwrap()
}

#[cfg(target_os = "linux")]
fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap()
}

#[test]
#[cfg(target_os = "linux")]
fn reviewed_hud_scaler_preserves_original_pixels_and_float_flags() {
    let _guard = env_lock();
    let script =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("engines/halo/tests/check_hud_upscale.py");
    for scalar in [false, true] {
        let mut command = std::process::Command::new("python3");
        command.arg(&script);
        if scalar {
            command.arg("--scalar");
        }
        #[cfg(target_arch = "x86")]
        command.args(["--cflag=-m32", "--cflag=-msse2", "--cflag=-mfpmath=sse"]);
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "scalar={scalar}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
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
    assert!(manifest.permissions.network);
    let offline = gamepkg::manifest::Manifest::parse(
        manifest_text()
            .replace("network = true", "network = false")
            .as_bytes(),
    )
    .unwrap();
    assert!(!offline.permissions.network);
    let catalog = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/ports.gamepkglist"),
    )
    .unwrap();
    let halo = catalog
        .split("id = \"com.gamepkg.halo\"")
        .nth(1)
        .expect("halo catalog entry");
    assert!(halo.contains("version = \"0.1.0\""));
    assert!(halo.contains("requires_rom = false"));
    assert!(halo.contains("i686"));
    assert!(halo.contains("/v0.1.6/halo.gamepkg"));
    let march = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("engines/halo/patches/0003-x86-arch-and-portable-march.patch"),
    )
    .unwrap();
    assert!(
        march.contains("return \"-msse2 -maes -mpclmul\""),
        "portable i686 Windows must enable AES-NI intrinsics for mbedtls"
    );
    let layout = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("engines/halo/patches/0004-lp64-long-width.patch"),
    )
    .unwrap();
    assert!(
        layout.contains("typedef int halo_s32;") && layout.contains("typedef long halo_s32;"),
        "Xbox long fields must stay 32 bits on LP64 and stay long where long is already 32 bits"
    );
    assert!(
        layout.contains("+#include \"halo_layout.h\""),
        "the Linux prefix must see halo_s32 before game headers"
    );
    assert!(
        !layout.contains("-#error the Linux port targets 32-bit x86"),
        "the 32-bit prefix guard stays until a 64-bit binary passes"
    );
    let pointers = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("engines/halo/patches/0005-halo-addr-pointer-fields.patch"),
    )
    .unwrap();
    assert!(
        pointers.contains("((halo_ty_") && pointers.contains("halo_ref("),
        "pointer fields in size-asserted structs load through halo_addr"
    );
    assert!(
        pointers.contains("#define halo_ref_any(value) ((halo_addr)(value))"),
        "stores into halo_addr stay constant expressions"
    );
    assert!(
        !pointers.contains("-#error the Linux port targets 32-bit x86")
            && !pointers.contains("-#error the Windows port targets 32-bit x86"),
        "patch 0005 leaves both prefix guards in place"
    );
    let fixups = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("engines/halo/patches/0006-lp64-layout-and-load-time-fixups.patch"),
    )
    .unwrap();
    assert!(
        fixups.contains("constructor(100)")
            && fixups.contains("#if defined(__LP64__) || defined(_WIN64)"),
        "pointer-to-halo_addr initializers are filled at load time on LP64 and Win64"
    );
    assert!(
        fixups.contains("-#error the Linux port targets 32-bit x86"),
        "the Linux prefix guard comes out only after linux-x86_64 passes"
    );
    assert!(
        fixups.contains("-#error the Windows port targets 32-bit x86"),
        "the Windows prefix guard comes out only after windows-x64 passes"
    );
    let clock = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("engines/halo/patches/0007-gamepkg-clock.patch"),
    )
    .unwrap();
    assert!(
        clock.contains("gamepkg_monotonic_time_ns")
            && clock.contains("gamepkg_wall_time")
            && clock.contains("__attribute__((weak))"),
        "the game clock goes through the host ABI, with a weak clock_gettime fallback"
    );
    let threads = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("engines/halo/patches/0009-gamepkg-threads.patch"),
    )
    .unwrap();
    assert!(
        threads.contains("gamepkg_thread_spawn")
            && threads.contains("gamepkg_thread_sleep_ms")
            && threads.contains("gamepkg_thread_yield")
            && threads.contains("__attribute__((weak))"),
        "CreateThread, SleepEx, and SwitchToThread go through the host ABI"
    );
    let directory = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("engines/halo/patches/0010-gamepkg-directory.patch"),
    )
    .unwrap();
    assert!(
        directory.contains("gamepkg_path_stat")
            && directory.contains("gamepkg_path_read")
            && directory.contains("gamepkg_dir_open")
            && directory.contains("gamepkg_dir_read")
            && directory.contains("__attribute__((weak))"),
        "map folder stat, listing, and header reads go through the host ABI"
    );
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
        (
            "hard_memory_bytes = 4294967296",
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
    let manifest = gamepkg::manifest::Manifest::parse(manifest_text().as_bytes()).unwrap();
    let layouts = &manifest.game_data.as_ref().unwrap().layouts;
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
    // An explicit image remains unambiguous even in a Downloads folder
    // containing disc images for several different games.
    let selected = disc.join("halo.iso");
    assert!(matches!(
        gamepkg::game_data::matches(&selected, layouts).unwrap(),
        gamepkg::game_data::DataMatch::Beside { file } if file == selected.canonicalize().unwrap()
    ));
    let wrong_extension = disc.join("notes.txt");
    std::fs::write(&wrong_extension, vec![0; 2048]).unwrap();
    assert!(gamepkg::game_data::matches(&wrong_extension, layouts).is_err());
    let short = disc.join("short.iso");
    std::fs::write(&short, b"short").unwrap();
    assert!(gamepkg::game_data::matches(&short, layouts).is_err());
    let uppercase = disc.join("Chosen (Rev 2).ISO");
    std::fs::write(&uppercase, vec![0; 8192]).unwrap();
    assert!(gamepkg::game_data::matches(&uppercase, layouts).is_ok());
    let mut bounded = layouts.clone();
    for layout in &mut bounded {
        layout.max_bytes = Some(4096);
    }
    assert!(gamepkg::game_data::matches(&uppercase, &bounded).is_err());
    let library = root.path().join("library");
    std::fs::create_dir(&library).unwrap();
    gamepkg::game_data::remember(&library, &manifest, &selected).unwrap();
    assert_eq!(
        gamepkg::game_data::load(&library, &manifest).unwrap(),
        Some(selected.canonicalize().unwrap())
    );
    assert!(gamepkg::game_data::remember(&library, &manifest, &wrong_extension).is_err());
    assert_eq!(
        gamepkg::game_data::load(&library, &manifest).unwrap(),
        Some(selected.canonicalize().unwrap())
    );
    let maps = root.path().join("maps-root");
    std::fs::create_dir_all(maps.join("maps")).unwrap();
    std::fs::write(maps.join("maps/ui.map"), b"not-a-real-map").unwrap();
    assert!(matches!(
        gamepkg::game_data::matches(&maps, layouts).unwrap(),
        gamepkg::game_data::DataMatch::Link { .. }
    ));
    std::fs::write(maps.join("also.iso"), vec![0; 2048]).unwrap();
    assert!(
        matches!(
            gamepkg::game_data::matches(&maps, layouts).unwrap(),
            gamepkg::game_data::DataMatch::Link { .. }
        ),
        "maps stay preferred when a disc image is also present"
    );
    let wrong = root.path().join("wrong");
    std::fs::create_dir(&wrong).unwrap();
    std::fs::write(wrong.join("notes.txt"), vec![0; 2048]).unwrap();
    assert!(gamepkg::game_data::matches(&wrong, layouts).is_err());
    #[cfg(unix)]
    {
        let linked_dir = root.path().join("linked-dir");
        std::fs::create_dir_all(linked_dir.join("real")).unwrap();
        std::fs::write(linked_dir.join("real/ui.map"), b"not-a-real-map").unwrap();
        std::os::unix::fs::symlink("real", linked_dir.join("maps")).unwrap();
        assert!(gamepkg::game_data::matches(&linked_dir, layouts).is_err());
        let linked_file = root.path().join("linked-file");
        std::fs::create_dir_all(linked_file.join("maps")).unwrap();
        std::fs::write(linked_file.join("maps/real.map"), b"not-a-real-map").unwrap();
        std::os::unix::fs::symlink("real.map", linked_file.join("maps/ui.map")).unwrap();
        assert!(gamepkg::game_data::matches(&linked_file, layouts).is_err());
        let escape = root.path().join("escape");
        std::fs::create_dir_all(escape.join("maps")).unwrap();
        std::fs::write(escape.join("maps/ui.map"), b"not-a-real-map").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", escape.join("maps/outside")).unwrap();
        let error = gamepkg::game_data::matches(&escape, layouts)
            .unwrap_err()
            .to_string();
        assert!(error.contains("leaves the game-data directory"), "{error}");
        let linked_disc = root.path().join("linked-disc");
        std::fs::create_dir(&linked_disc).unwrap();
        std::fs::write(linked_disc.join("real.iso"), vec![0; 2048]).unwrap();
        std::os::unix::fs::symlink("real.iso", linked_disc.join("halo.iso")).unwrap();
        assert!(gamepkg::game_data::matches(&linked_disc, layouts).is_err());
        assert!(gamepkg::game_data::matches(&linked_disc.join("halo.iso"), layouts).is_ok(),
            "an explicit player-selected link resolves to its canonical image; directory scanning still rejects links");
        assert!(gamepkg::game_data::matches(Path::new("/dev/null"), layouts).is_err());
        let inside = root.path().join("inside-link");
        std::fs::create_dir_all(inside.join("maps")).unwrap();
        std::fs::write(inside.join("maps/ui.map"), b"not-a-real-map").unwrap();
        std::os::unix::fs::symlink("ui.map", inside.join("maps/alias.map")).unwrap();
        assert!(matches!(
            gamepkg::game_data::matches(&inside, layouts).unwrap(),
            gamepkg::game_data::DataMatch::Link { .. }
        ));
    }
    let tiny = root.path().join("tiny");
    std::fs::create_dir(&tiny).unwrap();
    std::fs::write(tiny.join("halo.iso"), vec![0; 32]).unwrap();
    assert!(gamepkg::game_data::matches(&tiny, layouts).is_err());
    let huge_map = root.path().join("huge-map");
    std::fs::create_dir_all(huge_map.join("maps")).unwrap();
    let map = huge_map.join("maps/ui.map");
    std::fs::File::create(&map)
        .unwrap()
        .set_len(67_108_865)
        .unwrap();
    assert!(gamepkg::game_data::matches(&huge_map, layouts).is_err());
    let huge_disc = root.path().join("huge-disc");
    std::fs::create_dir(&huge_disc).unwrap();
    let disc_file = huge_disc.join("halo.iso");
    std::fs::File::create(&disc_file)
        .unwrap()
        .set_len(8_589_934_593)
        .unwrap();
    assert!(gamepkg::game_data::matches(&huge_disc, layouts).is_err());
    let nested = root.path().join("nested");
    std::fs::create_dir_all(nested.join("inner/maps")).unwrap();
    std::fs::write(nested.join("inner/maps/ui.map"), b"not-a-real-map").unwrap();
    let via_parent = nested.join("inner/extra/..");
    std::fs::create_dir_all(nested.join("inner/extra")).unwrap();
    assert!(matches!(
        gamepkg::game_data::matches(&via_parent, layouts).unwrap(),
        gamepkg::game_data::DataMatch::Link { .. }
    ));
}

#[test]
fn native_linux_defaults_to_the_gameplay_validated_target() {
    let failure = gamepkg::external_engine::select(engine(), Some("linux-x86_64"))
        .unwrap_err()
        .to_string();
    assert!(failure.contains("gameplay") && failure.contains("linux-i686"));
    assert!(gamepkg::external_engine::select(engine(), Some("linux-i686")).is_ok());
    #[cfg(target_os = "linux")]
    assert_eq!(
        gamepkg::external_engine::select(engine(), None).unwrap().id,
        "linux-i686"
    );
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
    gamepkg::native::compile_for(library.path(), "com.gamepkg.halo", Some("linux-i686")).unwrap();
    assert!(gamepkg::native::is_built(library.path(), "com.gamepkg.halo").unwrap());
    let report = gamepkg::integrity::check(library.path(), "com.gamepkg.halo").unwrap();
    assert!(report.contains("compiled game verified"), "{report}");
    let data = library.path().join("not-real");
    std::fs::create_dir_all(data.join("maps")).unwrap();
    std::fs::write(data.join("maps/ui.map"), b"placeholder").unwrap();
    gamepkg::game_data::remember_for_game(library.path(), "com.gamepkg.halo", &data).unwrap();
    let previous_home = std::env::var_os("GAMEPKG_HOME");
    let previous_config = std::env::var_os("GAMEPKG_CONFIG");
    let config = library.path().join("settings.toml");
    std::fs::write(&config, "memory_ceiling_mib = 3072\n").unwrap();
    std::env::set_var("GAMEPKG_HOME", library.path());
    std::env::set_var("GAMEPKG_CONFIG", &config);
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    std::env::set_var("SDL_AUDIODRIVER", "dummy");
    std::env::set_var("HALO_HIDDEN_WINDOW", "1");
    std::env::set_var("HALO_NULL_RENDERER", "1");
    std::env::set_var("HALO_EXIT_AFTER", "1");
    let error = gamepkg::external_engine::launch(library.path(), "com.gamepkg.halo")
        .unwrap_err()
        .to_string();
    std::env::remove_var("HALO_EXIT_AFTER");
    match previous_home {
        Some(home) => std::env::set_var("GAMEPKG_HOME", home),
        None => std::env::remove_var("GAMEPKG_HOME"),
    }
    match previous_config {
        Some(config) => std::env::set_var("GAMEPKG_CONFIG", config),
        None => std::env::remove_var("GAMEPKG_CONFIG"),
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

#[test]
#[cfg(target_os = "linux")]
fn normal_native_launch_requires_a_window() {
    let _guard = env_lock();
    let output = std::process::Command::new("python3")
        .arg("engines/halo/tests/check_video_failure.py")
        .output()
        .expect("run maintainer native-window policy fixture");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
