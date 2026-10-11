use ed25519_dalek::SigningKey;
use gamepkg::{archive::Package, pack::pack, verify::verify};
use std::{fs, io::Write, path::Path};
const SEED: [u8; 32] = [7; 32];
fn fixture(root: &Path) {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for entry in fs::read_dir(from).unwrap() {
            let e = entry.unwrap();
            if e.file_type().unwrap().is_dir() {
                copy(&e.path(), &to.join(e.file_name()));
            } else {
                fs::copy(e.path(), to.join(e.file_name())).unwrap();
            }
        }
    }
    copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/minimal")
            .as_path(),
        root,
    );
}
#[test]
fn signed_roundtrip_and_asset_access() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("game");
    fixture(&root);
    let out = temp.path().join("game.gamepkg");
    pack(&root, &out, &SEED).unwrap();
    let mut pkg = Package::open(&out).unwrap();
    assert_eq!(
        verify(
            &mut pkg,
            SigningKey::from_bytes(&SEED).verifying_key().as_bytes()
        )
        .unwrap()
        .id,
        "com.gamepkg.minimal"
    );
    assert_eq!(
        pkg.asset("hello.txt").unwrap(),
        b"Hello from an archived asset!\n"
    );
    assert!(pkg.asset("../manifest.toml").is_err());
    assert!(verify(
        &mut pkg,
        SigningKey::from_bytes(&[8; 32]).verifying_key().as_bytes()
    )
    .is_err());
    assert!(!temp.path().join("assets").exists());
}
#[test]
fn rejects_source_bypasses() {
    for source in [
        "pub fn x() { let _ = std::fs::read(\"x\"); }",
        "pub fn x() { let _ = std::net::TcpStream::connect(\"x\"); }",
        "pub const X: &[u8] = include_bytes!(\"x\");",
        "pub const X: &str = include_str!(\"x\");",
        "include!(\"x.rs\");",
        "pub fn x() { unsafe {} }",
        "#[path = \"outside.rs\"] mod x;",
        "extern \"C\" { fn x(); }",
        "mod outside;",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        fixture(&root);
        fs::write(root.join("source/src/lib.rs"), source).unwrap();
        let out = temp.path().join("bad.gamepkg");
        assert!(pack(&root, &out, &SEED).is_err(), "accepted {source}");
        assert!(!out.exists());
    }
}
#[test]
fn rejects_manifest_and_dependency_bypasses() {
    for cargo in [
        "[package]\nname='bad'\nversion='0.1.0'\nbuild='build.rs'\n",
        "[package]\nname='bad'\nversion='0.1.0'\nbuild=false\n[dependencies]\nevil={path='../evil'}\n",
        "[package]\nname='bad'\nversion='0.1.0'\nbuild=false\n[lib]\nproc-macro=true\n",
        "[package]\nname='bad'\nversion='0.1.0'\nbuild=false\n[dependencies]\nevil={git='https://invalid.test'}\n",
        "[package]\nname='bad'\nversion='0.1.0'\nbuild=false\n[dependencies]\nevil='1'\n",
    ] {
        let temp = tempfile::tempdir().unwrap(); let root = temp.path().join("game"); fixture(&root);
        fs::write(root.join("source/Cargo.toml"), cargo).unwrap();
        assert!(pack(&root, &temp.path().join("bad.gamepkg"), &SEED).is_err());
    }
}
#[test]
fn rejects_bad_layout_and_lockfile() {
    for path in [
        "source/build.rs",
        "shaders/bad.spv",
        "preview.av1.webm",
        "unexpected",
        "source/Cargo.lock",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        fixture(&root);
        fs::write(root.join(path), b"bad").unwrap();
        assert!(
            pack(&root, &temp.path().join("bad.gamepkg"), &SEED).is_err(),
            "accepted {path}"
        );
    }
}
#[test]
fn rejects_traversal_duplicates_and_symlinks() {
    use zip::{write::SimpleFileOptions, ZipWriter};
    for name in [
        "../escape",
        "/absolute",
        "assets/../escape",
        "C:drive",
        "assets\\escape",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let out = temp.path().join("bad.zip");
        let mut z = ZipWriter::new(fs::File::create(&out).unwrap());
        z.start_file(
            name,
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        z.write_all(b"x").unwrap();
        z.finish().unwrap();
        assert!(Package::open(&out).is_err());
    }
    #[cfg(unix)]
    {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        fixture(&root);
        std::os::unix::fs::symlink("/etc/passwd", root.join("assets/link")).unwrap();
        assert!(pack(&root, &temp.path().join("bad.gamepkg"), &SEED).is_err());
    }
}
#[test]
fn detects_tampering() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("game");
    fixture(&root);
    let out = temp.path().join("game.gamepkg");
    pack(&root, &out, &SEED).unwrap();
    let original = fs::read(&out).unwrap();
    // Rebuild the archive with a modified asset and the original signature.
    let mut pkg = Package::open(&out).unwrap();
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in pkg.entries.keys().cloned().collect::<Vec<_>>() {
        z.start_file(
            &name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        let data = if name == "assets/hello.txt" {
            b"tampered".to_vec()
        } else {
            pkg.read(&name).unwrap()
        };
        z.write_all(&data).unwrap();
    }
    let changed = z.finish().unwrap().into_inner();
    assert_ne!(original, changed);
    drop(pkg);
    fs::write(&out, changed).unwrap();
    assert!(verify(
        &mut Package::open(&out).unwrap(),
        SigningKey::from_bytes(&SEED).verifying_key().as_bytes()
    )
    .is_err());
}

#[test]
fn rejects_duplicate_names_and_oversized_entries() {
    let temp = tempfile::tempdir().unwrap();
    let out = temp.path().join("duplicate.zip");
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for name in ["assets/a", "assets/b"] {
        z.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        z.write_all(b"x").unwrap();
    }
    let mut bytes = z.finish().unwrap().into_inner();
    // Change both local and central directory names to create duplicate paths.
    for i in 0..bytes.len() - 8 {
        if &bytes[i..i + 8] == b"assets/b" {
            bytes[i + 7] = b'a';
        }
    }
    fs::write(&out, &bytes).unwrap();
    assert!(Package::open(&out).is_err());
    // Forge the first central directory entry's uncompressed size above the cap.
    let i = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
    bytes[i + 24..i + 28].copy_from_slice(&((gamepkg::archive::MAX_FILE + 1) as u32).to_le_bytes());
    fs::write(&out, bytes).unwrap();
    assert!(Package::open(&out).is_err());
}

#[test]
fn rejects_network_grants_and_preserves_existing_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("game");
    fixture(&root);
    let manifest = root.join("manifest.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest)
            .unwrap()
            .replace("network = \"none\"", "network = \"all\""),
    )
    .unwrap();
    let out = temp.path().join("bad.gamepkg");
    assert!(pack(&root, &out, &SEED).is_err());
    fs::write(&out, b"keep this").unwrap();
    assert!(pack(&root, &out, &SEED).is_err());
    assert_eq!(fs::read(&out).unwrap(), b"keep this");
}
