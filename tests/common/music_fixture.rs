pub fn signed_music_package(
    paths: &[std::path::PathBuf],
) -> (tempfile::TempDir, gamepkg::archive::Package) {
    let temporary = tempfile::tempdir().unwrap();
    let recipe = temporary.path().join("recipe");
    // Embed fixture inputs so native test binaries also run on Android without
    // the build machine's source checkout. All bytes match the minimal recipe.
    for (relative, bytes) in [
        (
            "manifest.toml",
            &include_bytes!("../../examples/minimal/manifest.toml")[..],
        ),
        (
            "LICENSE",
            &include_bytes!("../../examples/minimal/LICENSE")[..],
        ),
        (
            "NOTICE",
            &include_bytes!("../../examples/minimal/NOTICE")[..],
        ),
        (
            "CREDITS.md",
            &include_bytes!("../../examples/minimal/CREDITS.md")[..],
        ),
        (
            "thumbnail.png",
            &include_bytes!("../../examples/minimal/thumbnail.png")[..],
        ),
        (
            "source/Cargo.toml",
            &include_bytes!("../../examples/minimal/source/Cargo.toml")[..],
        ),
        (
            "source/Cargo.lock",
            &include_bytes!("../../examples/minimal/source/Cargo.lock")[..],
        ),
        (
            "source/src/lib.rs",
            &include_bytes!("../../examples/minimal/source/src/lib.rs")[..],
        ),
        (
            "shaders/main.slang",
            &include_bytes!("../../examples/minimal/shaders/main.slang")[..],
        ),
        (
            "assets/hello.txt",
            &include_bytes!("../../examples/minimal/assets/hello.txt")[..],
        ),
    ] {
        let target = recipe.join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, bytes).unwrap();
    }
    let manifest = recipe.join("manifest.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap()
        .replace("audio = false", "audio = true");
    std::fs::write(manifest, text).unwrap();
    std::fs::create_dir_all(recipe.join("assets/music")).unwrap();
    for path in paths {
        std::fs::copy(
            path,
            recipe.join("assets/music").join(path.file_name().unwrap()),
        )
        .unwrap();
    }
    let archive = temporary.path().join("private-music-fixture.gamepkg");
    // Synthetic fixture key only. No user or release signing key is read.
    gamepkg::pack::pack(&recipe, &archive, &[0x53; 32]).unwrap();
    let mut package = gamepkg::archive::Package::open(&archive).unwrap();
    let first = format!("music/{}", paths[0].file_name().unwrap().to_str().unwrap());
    let mut called = false;
    assert!(package
        .with_asset_reader(&first, 8 * 1048576, |_, _| {
            called = true;
            Ok(())
        })
        .is_err());
    assert!(!called);
    assert!(
        gamepkg::verify::verify_embedded(&mut package)
            .unwrap()
            .permissions
            .audio
    );
    (temporary, package)
}
