use std::process::Command;
#[test]
fn cli_pack_verify_and_asset_roundtrip() {
    let temp = tempfile::tempdir().unwrap();
    let key = temp.path().join("creator.key");
    let package = temp.path().join("sample.gamepkg");
    let asset = temp.path().join("hello.txt");
    let seed = [11; 32];
    std::fs::write(&key, hex::encode(seed)).unwrap();
    let public = hex::encode(
        ed25519_dalek::SigningKey::from_bytes(&seed)
            .verifying_key()
            .as_bytes(),
    );
    let exe = env!("CARGO_BIN_EXE_gamepkg");
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
