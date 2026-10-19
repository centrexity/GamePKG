use gamepkg::{archive::Package, pool::Pool};
#[test]
fn signed_references_resolve_to_private_snapshots_and_install_requires_blobs() {
    let root = tempfile::tempdir().unwrap();
    let library = root.path().join("library");
    let package_path = root.path().join("store.gamepkg");
    gamepkg::pack::pack_shared(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/minimal")),
        &package_path,
        &[9; 32],
        &library,
    )
    .unwrap();
    let mut package = Package::open(&package_path).unwrap();
    let manifest = gamepkg::verify::verify_embedded(&mut package).unwrap();
    assert!(!package.entries.contains_key("assets/hello.txt"));
    assert_eq!(manifest.shared_assets.len(), 1);
    assert!(package.asset("hello.txt").is_err());
    package.resolve_shared_assets(&library).unwrap();
    assert_eq!(package.asset_chunk("hello.txt", 6, 4).unwrap(), b"from");
    assert!(package.asset_chunk("../hello.txt", 0, 1).is_err());
    let reference = &manifest.shared_assets["hello.txt"];
    std::fs::remove_file(
        library
            .join("asset-pool")
            .join(format!("{}.blob", reference.sha256)),
    )
    .unwrap();
    assert_eq!(package.asset_chunk("hello.txt", 6, 4).unwrap(), b"from");
    assert!(gamepkg::registry::install(&library, &package_path, true).is_err());
    assert!(!library.join("com.gamepkg.minimal.gamepkg").exists());
    Pool::open(&library)
        .unwrap()
        .put(b"Hello from an archived asset!\n")
        .unwrap();
    gamepkg::registry::install(&library, &package_path, true).unwrap();
}
#[test]
fn reference_paths_hashes_and_sizes_are_validated() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap();
    for (path, hash, size) in [
        ("../escape", "a".repeat(64), 1),
        ("data", "A".repeat(64), 1),
        ("data", "a".repeat(64), gamepkg::archive::MAX_FILE + 1),
    ] {
        let input =
            format!("{text}\n[shared_assets.\"{path}\"]\nsha256 = \"{hash}\"\nsize = {size}\n");
        assert!(gamepkg::manifest::Manifest::parse(input.as_bytes()).is_err());
    }
}

#[test]
fn a_shared_digest_cannot_claim_conflicting_sizes() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap();
    let hash = "a".repeat(64);
    let input = format!("{text}\n[shared_assets.first]\nsha256 = \"{hash}\"\nsize = 1\n[shared_assets.second]\nsha256 = \"{hash}\"\nsize = 2\n");
    assert!(gamepkg::manifest::Manifest::parse(input.as_bytes())
        .unwrap_err()
        .to_string()
        .contains("conflicting sizes"));
}
