use std::process::Command;
#[test]
fn cli_pack_verify_and_asset_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("creator.key");
    let package = temp.path().join("sample.gamepkg");
    let asset = temp.path().join("hello.txt");
    let exe = env!("CARGO_BIN_EXE_gamepkg");
    let generated = Command::new(exe)
        .arg("keygen")
        .arg("-o")
        .arg(&key)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let seed = gamepkg::creator::read_seed(&key).unwrap();
    let public = hex::encode(
        ed25519_dalek::SigningKey::from_bytes(&seed)
            .verifying_key()
            .as_bytes(),
    );
    let report = String::from_utf8(generated.stdout).unwrap();
    assert!(report.contains(&public));
    assert!(!report.contains(&hex::encode(seed)));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let output = Command::new(exe)
        .args([
            "pack",
            concat!(env!("CARGO_MANIFEST_DIR"), "/examples/minimal"),
            "-o",
        ])
        .arg(&package)
        .arg("--key")
        .arg(&key)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut signed = gamepkg::archive::Package::open(&package).unwrap();
    let manifest = gamepkg::verify::verify_embedded(&mut signed).unwrap();
    assert_eq!(manifest.creator_key, public);
    assert!(!signed.entries.contains_key("creator.key"));
    assert!(Command::new(exe)
        .arg("verify")
        .arg(&package)
        .output()
        .unwrap()
        .status
        .success());
    assert!(Command::new(exe)
        .arg("verify")
        .arg(&package)
        .args(["--trusted-key", &public])
        .output()
        .unwrap()
        .status
        .success());
    assert!(!Command::new(exe)
        .arg("verify")
        .arg(&package)
        .args(["--trusted-key", "invalid"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(Command::new(exe)
        .arg("asset")
        .arg(&package)
        .args(["hello.txt", "--trusted-key", &public, "-o"])
        .arg(&asset)
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(
        std::fs::read(&asset).unwrap(),
        b"Hello from an archived asset!\n"
    );
    assert!(!Command::new(exe)
        .arg("asset")
        .arg(&package)
        .args(["../LICENSE", "--trusted-key", &public, "-o"])
        .arg(temp.path().join("escape"))
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn key_generation_preserves_existing_files_and_produces_distinct_keys() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.key");
    let second = temp.path().join("second.key");
    let public = gamepkg::creator::generate(&first).unwrap();
    let old = std::fs::read(&first).unwrap();
    assert!(gamepkg::creator::generate(&first).is_err());
    assert_eq!(std::fs::read(&first).unwrap(), old);
    assert_ne!(gamepkg::creator::generate(&second).unwrap(), public);
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 2);
    std::fs::write(&second, vec![b'a'; 257]).unwrap();
    assert!(gamepkg::creator::read_seed(&second).is_err());
    std::fs::write(&second, "abc").unwrap();
    assert!(gamepkg::creator::read_seed(&second).is_err());
    assert!(gamepkg::creator::read_seed(temp.path()).is_err());
}

#[cfg(unix)]
#[test]
fn key_generation_does_not_follow_an_existing_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    let link = temp.path().join("creator.key");
    std::fs::write(&target, "keep").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(gamepkg::creator::generate(&link).is_err());
    assert_eq!(std::fs::read_to_string(target).unwrap(), "keep");
    assert!(std::fs::symlink_metadata(link)
        .unwrap()
        .file_type()
        .is_symlink());
}
