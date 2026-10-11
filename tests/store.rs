use gamepkg::{
    pool::Pool,
    store::{self, Bundle},
};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
fn game(root: &Path, id: &str, seed: u8) -> PathBuf {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &to.join(entry.file_name()));
            } else {
                fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
    }
    let source = root.join(format!("source-{id}-{seed}"));
    copy(
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/minimal")),
        &source,
    );
    let manifest = source.join("manifest.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest)
            .unwrap()
            .replace("com.gamepkg.minimal", id),
    )
    .unwrap();
    let package = root.join(format!("{id}-{seed}.gamepkg"));
    gamepkg::pack::pack_shared(
        &source,
        &package,
        &[seed; 32],
        &root.join("creator-library"),
    )
    .unwrap();
    package
}
#[test]
fn portable_store_deduplicates_blobs_requires_trust_and_imports_idempotently() {
    let root = tempfile::tempdir().unwrap();
    let inputs = vec![
        game(root.path(), "game.one", 1),
        game(root.path(), "game.two", 2),
    ];
    let output = root.path().join("games.gamepkgstore");
    store::export(&root.path().join("creator-library"), &inputs, &output).unwrap();
    let mut archive = gamepkg::archive::Package::open(&output).unwrap();
    assert_eq!(
        archive
            .entries
            .keys()
            .filter(|name| name.starts_with("blobs/"))
            .count(),
        1
    );
    assert_eq!(
        archive
            .entries
            .keys()
            .filter(|name| name.starts_with("packages/"))
            .count(),
        2
    );
    let bundle = Bundle::open(&output).unwrap();
    assert_eq!(bundle.games().len(), 2);
    let destination = root.path().join("player-library");
    assert!(bundle.install(&destination, false).is_err());
    assert!(!destination.exists());
    assert_eq!(bundle.install(&destination, true).unwrap(), 2);
    assert_eq!(bundle.install(&destination, false).unwrap(), 0);
    assert_eq!(gamepkg::registry::list(&destination).unwrap().len(), 2);
    let manifest = &bundle.games()[0];
    let reference = &manifest.shared_assets["hello.txt"];
    assert_eq!(
        Pool::open(&destination).unwrap().read(reference).unwrap(),
        b"Hello from an archived asset!\n"
    );
    let original = fs::read(&output).unwrap();
    assert!(store::export(&root.path().join("creator-library"), &inputs, &output).is_err());
    assert_eq!(fs::read(output).unwrap(), original);
    // A missing creator blob must not publish a partial export.
    fs::remove_file(
        root.path()
            .join("creator-library/asset-pool")
            .join(format!("{}.blob", reference.sha256)),
    )
    .unwrap();
    let missing = root.path().join("missing.gamepkgstore");
    assert!(store::export(&root.path().join("creator-library"), &inputs, &missing).is_err());
    assert!(!missing.exists());
    let _ = archive.read("store.toml").unwrap();
}
#[test]
fn malformed_bundle_contents_are_rejected_before_installation() {
    let root = tempfile::tempdir().unwrap();
    let input = game(root.path(), "game.one", 3);
    let good = root.path().join("good.gamepkgstore");
    store::export(&root.path().join("creator-library"), &[input], &good).unwrap();
    for case in [
        "missing",
        "corrupt-blob",
        "corrupt-package",
        "unexpected",
        "wrong-version",
    ] {
        let bad = root.path().join(format!("{case}.gamepkgstore"));
        let mut original = zip::ZipArchive::new(fs::File::open(&good).unwrap()).unwrap();
        let mut writer = zip::ZipWriter::new(fs::File::create(&bad).unwrap());
        for index in 0..original.len() {
            let mut entry = original.by_index(index).unwrap();
            let name = entry.name().to_owned();
            if case == "missing" && name.starts_with("blobs/") {
                continue;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            if case == "corrupt-blob" && name.starts_with("blobs/") {
                bytes[0] ^= 1;
            }
            if case == "corrupt-package" && name.starts_with("packages/") {
                bytes[0] ^= 1;
            }
            if case == "wrong-version" && name == "store.toml" {
                bytes = b"format_version = 2\n".to_vec();
            }
            writer
                .start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(&bytes).unwrap();
        }
        if case == "unexpected" {
            writer
                .start_file("surprise", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"untrusted").unwrap();
        }
        writer.finish().unwrap();
        assert!(Bundle::open(&bad).is_err(), "accepted {case}");
    }
}
#[test]
fn creator_conflicts_prevent_the_entire_install_batch() {
    let root = tempfile::tempdir().unwrap();
    let old = game(root.path(), "game.z-existing", 4);
    let destination = root.path().join("player-library");
    let initial = root.path().join("initial.gamepkgstore");
    store::export(&root.path().join("creator-library"), &[old], &initial).unwrap();
    Bundle::open(&initial)
        .unwrap()
        .install(&destination, true)
        .unwrap();
    let new = game(root.path(), "game.a-new", 5);
    let changed_signer = game(root.path(), "game.z-existing", 6);
    let update = root.path().join("update.gamepkgstore");
    store::export(
        &root.path().join("creator-library"),
        &[new, changed_signer],
        &update,
    )
    .unwrap();
    assert!(Bundle::open(&update)
        .unwrap()
        .install(&destination, true)
        .is_err());
    assert_eq!(gamepkg::registry::list(&destination).unwrap().len(), 1);
    assert!(!destination.join("game.a-new.gamepkg").exists());
}
