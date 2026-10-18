pub fn signed_music_package(
    paths: &[std::path::PathBuf],
) -> (tempfile::TempDir, gamepkg::archive::Package) {
    let temporary = tempfile::tempdir().unwrap();
    let recipe = temporary.path().join("recipe");
    let minimal = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal");
    for relative in [
        "manifest.toml",
        "LICENSE",
        "NOTICE",
        "CREDITS.md",
        "thumbnail.png",
        "source/Cargo.toml",
        "source/Cargo.lock",
        "source/src/lib.rs",
        "shaders/main.slang",
        "assets/hello.txt",
    ] {
        let target = recipe.join(relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(minimal.join(relative), target).unwrap();
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
