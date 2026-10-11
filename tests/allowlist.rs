use gamepkg::allowlist::approved;
#[test]
fn only_the_pinned_no_std_graph_is_allowed() {
    let base = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/good/byteorder-game/source/"
    );
    let cargo: toml::Value =
        toml::from_str(&std::fs::read_to_string(format!("{base}Cargo.toml")).unwrap()).unwrap();
    let lock = std::fs::read(format!("{base}Cargo.lock")).unwrap();
    assert!(approved(&cargo, &lock).unwrap());
    for field in ["name", "version"] {
        let mut bad = cargo.clone();
        bad["package"].as_table_mut().unwrap().remove(field);
        assert!(approved(&bad, &lock).is_err());
    }
    for field in ["path", "git", "package", "artifact"] {
        let mut bad = cargo.clone();
        bad["dependencies"]["byteorder-lite"]
            .as_table_mut()
            .unwrap()
            .insert(field.into(), "untrusted".into());
        assert!(approved(&bad, &lock).is_err());
    }
    for version in ["0.1", "*", "=0.2.0"] {
        let mut bad = cargo.clone();
        bad["dependencies"]["byteorder-lite"]["version"] = version.into();
        assert!(approved(&bad, &lock).is_err());
    }
    let mut bad = cargo.clone();
    bad["dependencies"]["byteorder-lite"]["default-features"] = true.into();
    assert!(approved(&bad, &lock).is_err());
    let mut bad = cargo.clone();
    bad["dependencies"]["byteorder-lite"]
        .as_table_mut()
        .unwrap()
        .insert("features".into(), toml::Value::Array(vec!["std".into()]));
    assert!(approved(&bad, &lock).is_err());
    for change in [
        String::from_utf8(lock.clone())
            .unwrap()
            .replace(gamepkg::allowlist::CHECKSUM, &"0".repeat(64)),
        String::from_utf8(lock.clone()).unwrap().replace(
            "registry+https://github.com/rust-lang/crates.io-index",
            "git+https://example.com/untrusted",
        ),
    ] {
        assert!(approved(&cargo, change.as_bytes()).is_err());
    }
}

use std::process::Command;
#[test]
fn host_approvals_are_persistent_and_gate_signed_dependency_packages() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("settings.toml");
    let cli = env!("CARGO_BIN_EXE_gamepkg");
    let run = |args: &[&str]| {
        Command::new(cli)
            .env("GAMEPKG_CONFIG", &config)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(String::from_utf8_lossy(&run(&["allowlist", "list"]).stdout).contains("approved"));
    assert!(run(&["allowlist", "remove", "byteorder-lite"])
        .status
        .success());
    let before = std::fs::read(&config).unwrap();
    assert!(!run(&["allowlist", "add", "unreviewed-crate"])
        .status
        .success());
    assert_eq!(std::fs::read(&config).unwrap(), before);
    let key = temp.path().join("key");
    assert!(run(&["keygen", "-o", key.to_str().unwrap()])
        .status
        .success());
    let output = temp.path().join("game.gamepkg");
    let example = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/good/byteorder-game");
    let args = [
        "pack",
        example,
        "-o",
        output.to_str().unwrap(),
        "--key",
        key.to_str().unwrap(),
    ];
    let rejected = run(&args);
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("not approved by this host"),
        "{}",
        String::from_utf8_lossy(&rejected.stderr)
    );
    assert!(!output.exists());
    assert!(run(&["allowlist", "add", "byteorder-lite"])
        .status
        .success());
    assert!(run(&args).status.success());
    assert!(run(&["verify", output.to_str().unwrap()]).status.success());
    assert!(run(&["allowlist", "remove", "byteorder-lite"])
        .status
        .success());
    assert!(!run(&["verify", output.to_str().unwrap()]).status.success());
}
