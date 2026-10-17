//! Native verification/runtime support never authorizes an unisolated source build.
#[cfg(not(target_os = "linux"))]
#[test]
fn unsupported_platform_rejects_compilation_before_creating_a_build_tree() {
    use std::path::Path;
    let root = tempfile::tempdir().unwrap();
    let error = gamepkg::build::compile(root.path(), "com.gamepkg.not-installed").unwrap_err();
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

    let package_path = root.path().join("example.gamepkg");
    gamepkg::pack::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &package_path,
        &[7; 32],
    )
    .unwrap();
    let mut package = gamepkg::archive::Package::open(&package_path).unwrap();
    let manifest = gamepkg::verify::verify_embedded(&mut package).unwrap();
    let bytes = std::fs::read(package_path).unwrap();
    gamepkg::registry::install(root.path(), &root.path().join("example.gamepkg"), true).unwrap();
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
        .contains("isolated shader builds currently require Linux"));
    assert!(!root.path().join("shader-builds").exists());
    assert!(!root.path().join("builds").exists());

    let selector = root.path().join("cube2.gamepkg");
    gamepkg::pack::pack(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("ports/cube2"),
        &selector,
        &[7; 32],
    )
    .unwrap();
    gamepkg::registry::install(root.path(), &selector, true).unwrap();
    let error = gamepkg::native::compile(root.path(), "com.gamepkg.cube2").unwrap_err();
    assert!(error
        .to_string()
        .contains("Cube 2 currently requires Linux native process isolation"));
    assert!(!root.path().join("builds").exists());
}
