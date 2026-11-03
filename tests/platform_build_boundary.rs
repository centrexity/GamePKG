//! Native verification/runtime support never authorizes an unisolated source build.
#[cfg(not(target_os = "linux"))]
#[test]
fn unavailable_compiler_exchange_rejects_before_creating_output() {
    let root = tempfile::tempdir().unwrap();
    let error = gamepkg::build::compile(root.path(), "com.gamepkg.not-installed").unwrap_err();
    assert!(error.to_string().contains("package is not installed"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    let error = gamepkg::native::compile(root.path(), "com.gamepkg.not-installed").unwrap_err();
    assert!(error.to_string().contains("package is not installed"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    for compile in [
        gamepkg::build_worker::compile,
        gamepkg::build_worker::compile_transport,
    ] {
        let error = compile(
            &root.path().join("missing.gamepkgstore"),
            &root.path().join("exchange"),
            "glsl",
            true,
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("compiler worker currently requires Linux"));
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
#[test]
fn unavailable_native_and_shader_compilers_reject_before_creating_output() {
    let root = tempfile::tempdir().unwrap();
    let package_path = root.path().join("example.gamepkg");
    gamepkg::pack::pack(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &package_path,
        &[7; 32],
    )
    .unwrap();
    let mut package = gamepkg::archive::Package::open(&package_path).unwrap();
    let manifest = gamepkg::verify::verify_embedded(&mut package).unwrap();
    let bytes = std::fs::read(&package_path).unwrap();
    gamepkg::registry::install(root.path(), &package_path, true).unwrap();
    assert!(!gamepkg::native::build_supported(&manifest));
    assert!(!gamepkg::native::play_supported(&manifest));
    let error = gamepkg::native::compile(root.path(), &manifest.id).unwrap_err();
    assert!(error
        .to_string()
        .contains("native game isolation is not available on this platform"));
    assert!(!root.path().join("builds").exists());
    let error = gamepkg::shaders::compile(
        root.path(),
        &bytes,
        &mut package,
        &manifest,
        &Default::default(),
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("isolated shader builds require Linux or Windows"));
    assert!(!root.path().join("shader-builds").exists());
    assert!(!root.path().join("builds").exists());
}

#[cfg(not(target_os = "linux"))]
#[test]
fn unsupported_reviewed_engine_rejects_before_building_or_preparing_tools() {
    let root = tempfile::tempdir().unwrap();
    let selector = root.path().join("cube2.gamepkg");
    gamepkg::pack::pack(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/cube2"),
        &selector,
        &[7; 32],
    )
    .unwrap();
    let manifest = gamepkg::registry::install(root.path(), &selector, true).unwrap();
    assert!(!gamepkg::native::build_supported(&manifest));
    assert!(!gamepkg::native::play_supported(&manifest));
    let error = gamepkg::native::compile(root.path(), &manifest.id).unwrap_err();
    #[cfg(windows)]
    assert!(error
        .to_string()
        .contains("native build is not supported for this game on this platform"));
    #[cfg(not(windows))]
    assert!(error
        .to_string()
        .contains("Cube 2 currently requires Linux native process isolation"));
    for name in ["builds", "build-locks", "shader-builds", "native-sdk"] {
        assert!(!root.path().join(name).exists(), "unexpected {name}");
    }
}

#[cfg(all(windows, any(target_arch = "x86", target_arch = "x86_64")))]
#[test]
fn ordinary_windows_rust_games_admit_native_build_and_play() {
    // Real compile/worker/shader execution is verified by the explicit native
    // Windows fixtures; this fast check protects public admission policy.
    let manifest =
        gamepkg::manifest::Manifest::parse(include_bytes!("../examples/minimal/manifest.toml"))
            .unwrap();
    assert!(gamepkg::native::build_supported(&manifest));
    assert!(gamepkg::native::play_supported(&manifest));
}

#[cfg(target_os = "linux")]
#[test]
fn reviewed_takedawn_admission_matches_the_prepared_builder_architecture() {
    let manifest =
        gamepkg::manifest::Manifest::parse(include_bytes!("../ports/takedawn/manifest.toml"))
            .unwrap();
    let supported = cfg!(target_arch = "x86_64");
    assert_eq!(gamepkg::native::build_supported(&manifest), supported);
    assert_eq!(gamepkg::native::play_supported(&manifest), supported);
    // The reviewed engine's architecture does not restrict ordinary Rust games.
    let ordinary =
        gamepkg::manifest::Manifest::parse(include_bytes!("../examples/minimal/manifest.toml"))
            .unwrap();
    assert!(gamepkg::native::build_supported(&ordinary));
    assert!(gamepkg::native::play_supported(&ordinary));
}
