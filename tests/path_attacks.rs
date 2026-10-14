#[path = "support/path_cases.rs"]
mod cases;
#[path = "support/adversarial.rs"]
mod helper;
use gamepkg::{
    archive::{valid_path, Package},
    manifest::{Manifest, Shader},
    pool::{Pool, Reference},
};
use std::{fs, io::Write};

fn archive(names: &[&str]) -> (tempfile::TempDir, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("attack.gamepkg");
    let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
    for name in names {
        zip.start_file(
            *name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(b"asset bytes").unwrap();
    }
    zip.finish().unwrap();
    (temp, path)
}

#[test]
fn portable_path_policy_rejects_generated_traversal_and_device_spellings() {
    for path in cases::rejected_paths() {
        assert!(!valid_path(&path), "accepted dangerous path {path:?}");
    }
    assert!(valid_path(&"a".repeat(255)));
    assert!(valid_path(&vec!["a".repeat(31); 32].join("/")));
    for path in [
        "hello.txt",
        "a/b/file.rs",
        ".hidden",
        "a/file name.txt",
        "a/é.txt",
        "COM0",
        "COM10",
        "LPT0",
        "console.txt",
    ] {
        assert!(valid_path(path), "rejected safe path {path:?}");
    }
}

#[test]
fn malicious_zip_entry_names_are_rejected_before_contents_are_read() {
    for path in cases::rejected_paths()
        .into_iter()
        .filter(|path| !path.is_empty())
    {
        let (_temp, zip) = archive(&[&path]);
        assert!(Package::open(&zip).is_err(), "accepted ZIP entry {path:?}");
    }
}

#[test]
fn file_directory_and_case_collisions_cannot_alias_staged_files() {
    for names in [
        ["source/src/lib.rs", "source/src/LIB.rs"],
        ["assets/data", "assets/DATA"],
        ["source/src/Score/a.rs", "source/src/score/b.rs"],
        ["assets/data", "assets/data/secret"],
        ["assets/data/secret", "assets/data"],
        ["assets/DATA/secret", "assets/data"],
        ["assets/É.txt", "assets/é.txt"],
        ["source/src/é.rs", "source/src/e\u{301}.rs"],
        ["shaders/é.slang", "shaders/e\u{301}.slang"],
    ] {
        let (_temp, zip) = archive(&names);
        assert!(Package::open(&zip).is_err(), "accepted aliases {names:?}");
    }
}

#[test]
fn zip_symlink_entries_and_non_utf8_filesystem_names_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("links.gamepkg");
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.add_symlink(
        "assets/link",
        "../../secret",
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )
    .unwrap();
    zip.finish().unwrap();
    assert!(Package::open(&archive).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let source = temp.path().join("source");
        helper::write_fixture(&helper::fixture("examples/minimal"), &source);
        fs::write(
            source
                .join("assets")
                .join(std::ffi::OsString::from_vec(b"bad\xff.txt".to_vec())),
            b"bytes",
        )
        .unwrap();
        let output = temp.path().join("bad.gamepkg");
        assert!(gamepkg::pack::pack(&source, &output, &helper::SEED)
            .unwrap_err()
            .to_string()
            .contains("non UTF-8 path"));
        assert!(!output.exists());
    }
}

#[test]
fn encoded_and_unicode_lookalikes_are_exact_keys_never_host_paths() {
    let (_temp, zip) = archive(&["assets/a/hello.txt", "manifest.toml"]);
    let mut package = Package::open(&zip).unwrap();
    for path in cases::literal_paths() {
        assert!(
            valid_path(path),
            "literal should be a safe exact key: {path:?}"
        );
        assert!(
            package.asset(path).is_err(),
            "normalized asset lookup {path:?}"
        );
    }
    assert_eq!(package.asset("a/hello.txt").unwrap(), b"asset bytes");
    for path in cases::rejected_paths() {
        assert!(package.read(&path).is_err(), "raw read accepted {path:?}");
        assert!(package.asset(&path).is_err(), "asset accepted {path:?}");
        assert!(
            package.asset_chunk(&path, 0, 8).is_err(),
            "chunk accepted {path:?}"
        );
    }
    // Even a harmless encoded key remains usable only by its exact spelling.
    let (_temp, zip) = archive(&["assets/%2e%2e/hello.txt"]);
    let mut package = Package::open(&zip).unwrap();
    assert_eq!(package.asset("%2e%2e/hello.txt").unwrap(), b"asset bytes");
    assert!(package.asset("../hello.txt").is_err());
}

#[test]
fn manifests_and_pool_references_cannot_turn_virtual_names_into_host_paths() {
    let base = fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap();
    let manifest = Manifest::parse(&base).unwrap();
    let root = tempfile::tempdir().unwrap();
    let pool = Pool::open(root.path()).unwrap();
    for path in cases::rejected_paths() {
        let mut candidate = manifest.clone();
        candidate.shared_assets.insert(
            path.clone(),
            Reference {
                sha256: "0".repeat(64),
                size: 0,
            },
        );
        assert!(
            Manifest::parse(toml::to_string(&candidate).unwrap().as_bytes()).is_err(),
            "shared path {path:?}"
        );
        let shader = Shader {
            source: format!("shaders/{path}/main.slang"),
            entry: "main".into(),
            stage: "fragment".into(),
        };
        assert!(shader.validate().is_err(), "shader path {path:?}");
        assert!(pool
            .read(&Reference {
                sha256: path,
                size: 0
            })
            .is_err());
    }
    for id in [
        "CON", "nul.txt", "COM1", "LPT²", "game.", ".", "..", "../game", "C:game",
    ] {
        let mut candidate = manifest.clone();
        candidate.id = id.into();
        assert!(
            Manifest::parse(toml::to_string(&candidate).unwrap().as_bytes()).is_err(),
            "host directory ID {id:?}"
        );
    }
    for id in [
        "CON", "NUL.txt", "game.", "..", "../game", "/game", "C:game",
    ] {
        assert!(
            gamepkg::registry::installed_manifest(root.path(), id)
                .unwrap_err()
                .to_string()
                .contains("invalid package ID"),
            "registry {id:?}"
        );
    }
    assert_eq!(
        fs::read_dir(root.path().join("asset-pool"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn verified_embedded_and_shared_assets_never_escape_their_exact_signed_names() {
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path().join("secret");
    fs::write(&outside, b"host sentinel").unwrap();
    let mut files = helper::fixture("examples/minimal");
    files.insert(
        "assets/%2e%2e/hello.txt".into(),
        b"literal encoded asset".to_vec(),
    );
    let source = temp.path().join("source");
    helper::write_fixture(&files, &source);
    // Exercise both our normal packer and an independently signed archive.
    let signed = helper::sign(&files, temp.path());
    let embedded = temp.path().join("embedded.gamepkg");
    gamepkg::pack::pack(&source, &embedded, &helper::SEED).unwrap();
    let shared = temp.path().join("shared.gamepkg");
    let library = temp.path().join("library");
    gamepkg::pack::pack_shared(&source, &shared, &helper::SEED, &library).unwrap();
    for path in [embedded, shared, signed] {
        let mut package = Package::open(&path).unwrap();
        gamepkg::verify::verify_embedded(&mut package).unwrap();
        package.resolve_shared_assets(&library).unwrap();
        assert_eq!(package.asset_chunk("hello.txt", 0, 5).unwrap(), b"Hello");
        assert_eq!(
            package.asset_chunk("%2e%2e/hello.txt", 0, 7).unwrap(),
            b"literal"
        );
        for attack in cases::rejected_paths().into_iter().chain(
            cases::literal_paths()
                .iter()
                .filter(|p| **p != "%2e%2e/hello.txt")
                .map(|p| p.to_string()),
        ) {
            assert!(
                package.asset(&attack).is_err(),
                "asset {attack:?}, package {path:?}"
            );
            for offset in [0, 1, u64::MAX] {
                assert!(
                    package.asset_chunk(&attack, offset, 8).is_err(),
                    "chunk {attack:?}, {offset}, {path:?}"
                );
            }
        }
        assert!(package.asset_chunk("hello.txt", u64::MAX, 8).is_err());
        assert!(package.asset_chunk("hello.txt", 0, 65537).is_err());
        assert_eq!(package.asset_chunk("hello.txt", 0, 5).unwrap(), b"Hello");
        assert_eq!(fs::read(&outside).unwrap(), b"host sentinel");
    }
}
