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
    let package_digest = gamepkg::verify::digest(&std::fs::read(&package_path).unwrap());
    gamepkg::registry::install(root.path(), &package_path, true).unwrap();
    assert!(!gamepkg::native::build_supported(&manifest));
    assert!(!gamepkg::native::play_supported(&manifest));
    let error = gamepkg::native::compile(root.path(), &manifest.id).unwrap_err();
    assert!(error.to_string().contains("requires migration"));
    assert!(!root.path().join("builds").exists());
    let error = gamepkg::shaders::compile(
        root.path(),
        &package_digest,
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
    assert!(error.to_string().contains("requires migration"));
    for name in ["builds", "build-locks", "shader-builds", "native-sdk"] {
        assert!(!root.path().join(name).exists(), "unexpected {name}");
    }
}

#[test]
fn desktop_native_build_and_play_require_the_compatibility_profiles() {
    let old = include_str!("../examples/minimal/manifest.toml");
    let ordinary = gamepkg::manifest::Manifest::parse(old.as_bytes()).unwrap();
    assert!(!gamepkg::native::build_supported(&ordinary));
    assert!(!gamepkg::native::play_supported(&ordinary));
    for profile in ["native_tick", "native_services"] {
        let text = format!("{profile} = true\n{old}");
        let manifest = gamepkg::manifest::Manifest::parse(text.as_bytes()).unwrap();
        let has_runtime = cfg!(all(
            feature = "gui",
            target_os = "linux",
            target_arch = "x86_64"
        )) || (profile == "native_services"
            && cfg!(all(
                feature = "gui",
                windows,
                any(target_arch = "x86", target_arch = "x86_64")
            )));
        assert_eq!(gamepkg::native::build_supported(&manifest), has_runtime);
        assert_eq!(gamepkg::native::play_supported(&manifest), has_runtime);
    }
    let indexed = format!(
        "native_services = true\nnative_graphics = \"vertex-color-v1\"\n{old}\n\
         [[shaders]]\nsource = \"shaders/scene.slang\"\nentry = \"vertexMain\"\nstage = \"vertex\"\n\
         [[shaders]]\nsource = \"shaders/scene.slang\"\nentry = \"fragmentMain\"\nstage = \"fragment\"\n"
    );
    let manifest = gamepkg::manifest::Manifest::parse(indexed.as_bytes()).unwrap();
    let textured = format!(
        "native_services = true\nnative_graphics = \"textured-v1\"\n{old}\n\
         [[shaders]]\nsource = \"shaders/textured_v1.slang\"\nentry = \"vertexMain\"\nstage = \"vertex\"\n\
         [[shaders]]\nsource = \"shaders/textured_v1.slang\"\nentry = \"fragmentMain\"\nstage = \"fragment\"\n"
    );
    let textured_manifest = gamepkg::manifest::Manifest::parse(textured.as_bytes()).unwrap();
    let textured_v2 = format!(
        "native_services = true\nnative_graphics = \"textured-v2\"\n{old}\n\
         [[shaders]]\nsource = \"shaders/textured_v2.slang\"\nentry = \"vertexMain\"\nstage = \"vertex\"\n\
         [[shaders]]\nsource = \"shaders/textured_v2.slang\"\nentry = \"fragmentMain\"\nstage = \"fragment\"\n"
    );
    let textured_v2_manifest = gamepkg::manifest::Manifest::parse(textured_v2.as_bytes()).unwrap();
    let has_runtime = cfg!(all(
        feature = "gui",
        any(
            all(windows, any(target_arch = "x86", target_arch = "x86_64")),
            all(target_os = "linux", target_arch = "x86_64")
        )
    ));
    #[cfg(windows)]
    let has_runtime = has_runtime && gamepkg_takedawn_windows_runner::supports_amd64_tools();
    assert_eq!(gamepkg::native::build_supported(&manifest), has_runtime);
    assert_eq!(gamepkg::native::play_supported(&manifest), has_runtime);
    assert_eq!(
        gamepkg::native::build_supported(&textured_manifest),
        has_runtime
    );
    assert_eq!(
        gamepkg::native::play_supported(&textured_manifest),
        has_runtime
    );
    assert_eq!(
        gamepkg::native::build_supported(&textured_v2_manifest),
        has_runtime
    );
    assert_eq!(
        gamepkg::native::play_supported(&textured_v2_manifest),
        has_runtime
    );
    #[cfg(windows)]
    {
        let online = indexed.replace("network = \"none\"", "network = true");
        let online = gamepkg::manifest::Manifest::parse(online.as_bytes()).unwrap();
        assert!(!gamepkg::native::build_supported(&online));
        assert!(!gamepkg::native::play_supported(&online));
    }
}

#[test]
fn obsolete_native_cache_cannot_enable_an_unmigrated_package() {
    let root = tempfile::tempdir().unwrap();
    let library = root.path().join("library");
    let archive = root.path().join("ordinary.gamepkg");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal");
    gamepkg::pack::pack(&source, &archive, &[15; 32]).unwrap();
    let manifest = gamepkg::registry::install(&library, &archive, true).unwrap();
    let cache = gamepkg::native::cache_path(&library, &manifest.id).unwrap();
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, b"retired worker").unwrap();
    assert!(!gamepkg::native::is_built(&library, &manifest.id).unwrap());
    let result = gamepkg::native::compile(&library, &manifest.id);
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("requires migration"));
    let settings = gamepkg::settings::Settings::default();
    let result =
        gamepkg::native::play_services(&library, &manifest.id, &[], &settings, Default::default());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("requires migration"));
    assert_eq!(std::fs::read(cache).unwrap(), b"retired worker");
}

#[cfg(target_os = "linux")]
#[test]
fn unmigrated_takedawn_is_blocked_even_on_its_prepared_architecture() {
    let manifest =
        gamepkg::manifest::Manifest::parse(include_bytes!("../ports/takedawn/manifest.toml"))
            .unwrap();
    assert!(!gamepkg::native::build_supported(&manifest));
    assert!(!gamepkg::native::play_supported(&manifest));
}
