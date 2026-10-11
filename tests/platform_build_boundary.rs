//! Native verification/runtime support never authorizes an unisolated source build.
#[cfg(not(target_os = "linux"))]
#[test]
fn unsupported_platform_rejects_compilation_before_creating_a_build_tree() {
    use std::path::Path;
    let root = tempfile::tempdir().unwrap();
    let error = gamepkg::build::compile(root.path(), "com.gamepkg.not-installed").unwrap_err();
    assert!(error
        .to_string()
        .contains("isolated builds currently require Linux"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
    let error = gamepkg::build_worker::compile(
        &root.path().join("missing.gamepkgstore"),
        &root.path().join("exchange"),
        "glsl",
        true,
        true,
    )
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("compiler worker currently requires Linux"));
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);

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
}
