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
    assert_eq!(pkg.asset_chunk("hello.txt", 6, 4).unwrap(), b"from");
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
        "pub unsafe fn x() {}",
        "pub extern \"C\" fn x() {}",
        "#[path = \"outside.rs\"] mod x;",
        "extern \"C\" { fn x(); }",
        "mod outside;",
        "use std::include_bytes as bytes; pub const X: &[u8] = bytes!(\"x\");",
        "use std::fs as files; pub fn x() { let _ = files::read(\"x\"); }",
        "pub fn x() { let _ = std::r#fs::read(\"x\"); }",
        "macro_rules! hidden { () => { include_bytes!(\"x\") }; }",
        "macro_rules! hidden { () => { unsafe { } }; }",
        "macro_rules! hidden { () => { #[path=\"outside.rs\"] mod x; }; }",
        "macro_rules! hidden { () => { extern \"C\" { fn x(); } }; }",
        "macro_rules! hidden { () => { std::fs::read(\"x\") }; }",
        "macro_rules! hidden { () => { mod outside; }; }",
        "macro_rules! hidden { ($name:ident) => { $name!(\"x\") }; } pub const X: &[u8] = hidden!(include_bytes);",
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
fn rejects_invalid_network_permission_and_preserves_existing_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("game");
    fixture(&root);
    let manifest = root.join("manifest.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest)
            .unwrap()
            .replace("network = \"none\"", "network = \"sometimes\""),
    )
    .unwrap();
    let out = temp.path().join("bad.gamepkg");
    assert!(pack(&root, &out, &SEED).is_err());
    fs::write(&out, b"keep this").unwrap();
    assert!(pack(&root, &out, &SEED).is_err());
    assert_eq!(fs::read(&out).unwrap(), b"keep this");
}

#[test]
fn rejects_shader_assembly_and_macro_injection_without_publishing() {
    for source in [
        "float x() { return spirv_asm { result: $float = OpUndef; }; }",
        "#define ASM spirv_asm\nfloat x() { return ASM {}; }",
        "#define JOIN(a,b) a ## b\nJOIN(spirv_,asm)",
        "#define JOIN(a,b) a #/**/# b\nJOIN(spirv_,asm)",
        "spirv_\\\nasm {}",
        "spirv_\\\r\nasm {}",
        "void x() { __intrinsic_asm \"arbitrary code\"; }",
        "__requirePrelude(\"arbitrary code\")",
        "__target_intrinsic(glsl, \"arbitrary code\")",
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        fixture(&root);
        fs::write(root.join("shaders/unused.slang"), source).unwrap();
        let output = temp.path().join("bad.gamepkg");
        fs::write(&output, b"previous package").unwrap();
        assert!(pack(&root, &output, &SEED).is_err(), "accepted {source}");
        assert_eq!(fs::read(output).unwrap(), b"previous package");
    }
}

#[test]
fn macros_are_rejected_but_literal_text_remains_accepted() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("game");
    fixture(&root);
    let source = root.join("source/src/lib.rs");
    fs::write(
        &source,
        r#"macro_rules! add { ($x:expr) => { $x + 1 }; }
        pub fn game_tick(_: u32,state:u64)->u64 { add!(state) }"#,
    )
    .unwrap();
    let output = temp.path().join("ordinary.gamepkg");
    assert!(pack(&root, &output, &SEED).is_err());
    assert!(!output.exists());
    fs::write(
        &source,
        r#"pub fn game_tick(_: u32,state:u64)->u64 {
        let text = "unsafe std::fs include_bytes mod #";
        state + text.len() as u64
    }"#,
    )
    .unwrap();
    pack(&root, &output, &SEED).unwrap();
}

#[test]
fn rejects_understated_or_overstated_decoded_entry_sizes() {
    for method in [zip::CompressionMethod::Stored, zip::CompressionMethod::Zstd] {
        for declared in [0u32, 2, 100] {
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("size.zip");
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file(
                    "assets/a",
                    zip::write::SimpleFileOptions::default().compression_method(method),
                )
                .unwrap();
            writer.write_all(b"content").unwrap();
            let mut bytes = writer.finish().unwrap().into_inner();
            let central = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
            bytes[central + 24..central + 28].copy_from_slice(&declared.to_le_bytes());
            fs::write(&path, bytes).unwrap();
            if let Ok(mut package) = Package::open(&path) {
                assert!(
                    package.asset("a").is_err(),
                    "accepted {method:?} declared={declared}"
                );
            }
        }
    }
}

#[test]
fn malformed_cargo_identity_and_editions_fail_before_signing() {
    for (name, edition) in [
        ("valid", "edition = 2021"),
        ("valid", "edition = \"2099\""),
        ("valid", "edition = { workspace = true }"),
        ("9invalid", "edition = \"2021\""),
        ("invalid.name", "edition = \"2021\""),
        ("", "edition = \"2021\""),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        fixture(&root);
        fs::write(
            root.join("source/Cargo.toml"),
            format!("[package]\nname = {name:?}\nversion = \"0.1.0\"\nbuild = false\n{edition}\n"),
        )
        .unwrap();
        fs::write(
            root.join("source/Cargo.lock"),
            format!("version = 4\n[[package]]\nname = {name:?}\nversion = \"0.1.0\"\n"),
        )
        .unwrap();
        let manifest_path = root.join("manifest.toml");
        let mut manifest: toml::Value =
            toml::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["lockfile_sha256"] = toml::Value::String(gamepkg::verify::digest(
            &fs::read(root.join("source/Cargo.lock")).unwrap(),
        ));
        fs::write(manifest_path, toml::to_string(&manifest).unwrap()).unwrap();
        let output = temp.path().join("bad.gamepkg");
        let error = pack(&root, &output, &SEED).unwrap_err().to_string();
        assert!(
            error.contains("edition") || error.contains("package name"),
            "{error}"
        );
        assert!(!output.exists());
    }
}

#[test]
fn supported_cargo_editions_and_default_remain_accepted() {
    for edition in [None, Some("2015"), Some("2018"), Some("2021"), Some("2024")] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("game");
        fixture(&root);
        let cargo = root.join("source/Cargo.toml");
        let text = fs::read_to_string(&cargo).unwrap();
        let mut value: toml::Value = toml::from_str(&text).unwrap();
        let package = value["package"].as_table_mut().unwrap();
        if let Some(edition) = edition {
            package.insert("edition".into(), toml::Value::String(edition.into()));
        } else {
            package.remove("edition");
        }
        fs::write(cargo, toml::to_string(&value).unwrap()).unwrap();
        pack(&root, &temp.path().join("good.gamepkg"), &SEED).unwrap();
    }
}
