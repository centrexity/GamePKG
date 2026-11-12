//! Shared signed native SDK contract exercised on real Linux and Windows workers.
use std::path::Path;

pub(super) fn verify_signed_sdk(root: &Path, settings: &crate::settings::Settings) {
    let source = root.join("signed source");
    for name in [
        "manifest.toml",
        "thumbnail.png",
        "LICENSE",
        "NOTICE",
        "CREDITS.md",
        "source/Cargo.toml",
        "source/Cargo.lock",
    ] {
        let path = source.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("examples/minimal")
                .join(name),
            path,
        )
        .unwrap();
    }
    let manifest_path = source.join("manifest.toml");
    let text = std::fs::read_to_string(&manifest_path).unwrap();
    std::fs::write(&manifest_path, text.replace("save = false", "save = true")).unwrap();
    std::fs::create_dir_all(source.join("assets")).unwrap();
    std::fs::write(source.join("assets/fixture.bin"), [0, 255, 65, 33]).unwrap();
    std::fs::create_dir_all(source.join("source/src")).unwrap();
    std::fs::write(
        source.join("source/src/lib.rs"),
        r#"pub fn game_tick(input:u32,state:u64)->u64 {
let mut asset = [0;4];
if gamepkg::asset_read("fixture.bin",0,&mut asset) != Some(4)
    || asset != [0,255,65,33] { return 0xffff000000000001; }
let mut saved = [0;8];
let _ = gamepkg::save_read(0,&mut saved);
let previous = u64::from_le_bytes(saved);
if state != 0 && previous != state { return 0xffff000000000002; }
let next = previous + (u64::from(input) << 32);
if !gamepkg::save_write(&u64::to_le_bytes(next)) { return 0xffff000000000003; }
let _ = gamepkg::draw_clear(0x123456ff);
next
}"#,
    )
    .unwrap();
    let package_path = root.join("signed.gamepkg");
    crate::pack::pack(&source, &package_path, &[97; 32]).unwrap();
    let manifest = crate::registry::install(root, &package_path, true).unwrap();
    assert!(manifest.permissions.save);
    super::build::compile_with_settings(root, &manifest.id, settings).unwrap();
    let digest = crate::verify::digest(&std::fs::read(&package_path).unwrap());
    let built = super::path_for_digest(root, &digest);
    let mut bytes = std::fs::read(&built).unwrap();
    super::validate_executable(&bytes).unwrap();
    #[cfg(target_os = "linux")]
    {
        assert_eq!(bytes[4], if usize::BITS == 32 { 1 } else { 2 });
        let mut wrong_width = bytes.clone();
        wrong_width[4] = if bytes[4] == 1 { 2 } else { 1 };
        assert!(super::validate_executable(&wrong_width)
            .unwrap_err()
            .to_string()
            .contains("pointer width"));
        println!(
            "Verified native ELF{} build and rejection of a mismatched-width artifact",
            usize::BITS
        );
    }

    let open = || {
        let mut package = crate::archive::Package::open(&package_path).unwrap();
        crate::verify::verify_embedded(&mut package).unwrap();
        package.resolve_shared_assets(root).unwrap();
        package
    };
    let mut session = super::Session::new(
        root,
        &digest,
        open(),
        manifest.clone(),
        Vec::new(),
        settings,
        0,
    )
    .unwrap();
    assert_eq!(session.step(1, 0, &[]).unwrap(), 1u64 << 32);
    assert_eq!(session.step(3, 1u64 << 32, &[]).unwrap(), 4u64 << 32);
    drop(session);
    let mut session = super::Session::new(
        root,
        &digest,
        open(),
        manifest.clone(),
        Vec::new(),
        settings,
        0,
    )
    .unwrap();
    assert_eq!(session.step(2, 4u64 << 32, &[]).unwrap(), 6u64 << 32);
    drop(session);
    assert_eq!(
        crate::saves::Save::open_for_game(root, &manifest)
            .unwrap()
            .bytes,
        (6u64 << 32).to_le_bytes()
    );
    assert!(super::build_supported(&manifest));
    let mut game =
        crate::runtime::Game::load_target_configured(root, &manifest.id, &[], None, settings)
            .unwrap();
    assert!(game.custom_renderer);
    assert_eq!(game.step(5).unwrap(), 11u64 << 32);
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(game.step(1).unwrap(), 12u64 << 32);
    drop(game);
    assert!(
        crate::progress::load(root, &manifest.id, &manifest.creator_key)
            .unwrap()
            .played_ms
            > 0
    );
    assert_eq!(
        crate::saves::Save::open_for_game(root, &manifest)
            .unwrap()
            .bytes,
        (12u64 << 32).to_le_bytes()
    );
    println!("Verified normal game loader defaults to native playback, restores scoped saves and persists play history");
    bytes.extend_from_slice(b"corrupt");
    std::fs::write(&built, bytes).unwrap();
    assert!(super::Session::new(
        root,
        &digest,
        open(),
        manifest.clone(),
        Vec::new(),
        settings,
        0
    )
    .is_err());
    assert!(
        crate::runtime::Game::load_target_configured(root, &manifest.id, &[], None, settings)
            .is_err()
    );
    println!("Verified signed source package, native-only Build, generated SDK assets/drawing/ticks, scoped saves across fresh sessions and corrupted native-build rejection through the production session.");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "executes the isolated native compiler and game processes"]
fn signed_native_sdk_assets_saves_and_integrity() {
    let root = tempfile::tempdir().unwrap();
    verify_signed_sdk(root.path(), &crate::settings::Settings::default());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Clang 18, matching native system libraries and bubblewrap"]
fn reviewed_c_engines_build_for_app_width() {
    let root = tempfile::tempdir().unwrap();
    let settings = crate::settings::Settings::default();
    for name in ["doom", "zelda3"] {
        let package = root.path().join(format!("{name}.gamepkg"));
        crate::pack::pack(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("ports")
                .join(name),
            &package,
            &[119; 32],
        )
        .unwrap();
        let manifest = crate::registry::install(root.path(), &package, true).unwrap();
        super::build::compile_with_settings(root.path(), &manifest.id, &settings).unwrap();
        let digest = crate::verify::digest(&std::fs::read(&package).unwrap());
        let built = super::path_for_digest(root.path(), &digest);
        let bytes = std::fs::read(&built).unwrap();
        super::validate_executable(&bytes).unwrap();
        assert_eq!(bytes[4], if usize::BITS == 32 { 1 } else { 2 });
        crate::integrity::verify_build(&built, &bytes).unwrap();
        println!("Verified optimized signed native {name} ELF{} engine Build and integrity; no original ROM/media or gameplay used", usize::BITS);
    }
}
