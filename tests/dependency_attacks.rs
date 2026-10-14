#[path = "support/adversarial.rs"]
mod helper;
use gamepkg::{allowlist::approved, archive::Package};
use std::process::Command;

fn base() -> (toml::Value, Vec<u8>) {
    let files = helper::fixture("examples/good/byteorder-game");
    (
        toml::from_str(std::str::from_utf8(&files["source/Cargo.toml"]).unwrap()).unwrap(),
        files["source/Cargo.lock"].clone(),
    )
}

fn cargo_attacks() -> Vec<(String, toml::Value)> {
    let (base, _) = base();
    let mut attacks = Vec::new();
    for name in [
        "serde",
        "libc",
        "std",
        "tokio",
        "byteorder_lite",
        "Byteorder-lite",
        "byteorder-lite ",
        "byteorder-lite\u{200b}",
        "byteorder-lіte",
    ] {
        let mut cargo = base.clone();
        let dependency = cargo["dependencies"]["byteorder-lite"].clone();
        cargo["dependencies"]
            .as_table_mut()
            .unwrap()
            .insert(name.into(), dependency);
        attacks.push((format!("unapproved name {name:?}"), cargo));
    }
    for version in [
        "0.1.0",
        "^0.1.0",
        "~0.1.0",
        ">=0.1.0",
        "*",
        "=0.1.1",
        "=0.1.0 ",
        "=0.1.0+evil",
    ] {
        let mut cargo = base.clone();
        cargo["dependencies"]["byteorder-lite"]["version"] = version.into();
        attacks.push((format!("version {version:?}"), cargo));
    }
    for (key, value) in [
        ("path", "../../outside"),
        ("git", "https://example.invalid/crate"),
        ("registry", "evil"),
        ("registry-index", "https://example.invalid/index"),
        ("package", "unapproved"),
        ("branch", "main"),
        ("tag", "v1"),
        ("rev", "deadbeef"),
        ("artifact", "bin"),
        ("target", "x86_64-unknown-linux-gnu"),
    ] {
        let mut cargo = base.clone();
        cargo["dependencies"]["byteorder-lite"]
            .as_table_mut()
            .unwrap()
            .insert(key.into(), value.into());
        attacks.push((format!("source override {key}"), cargo));
    }
    for (key, value) in [
        ("optional", toml::Value::Boolean(true)),
        ("public", true.into()),
        ("default-features", true.into()),
        ("default_features", false.into()),
        ("features", toml::Value::Array(vec!["std".into()])),
        ("features", "std".into()),
        ("features", false.into()),
        ("version", toml::Value::Integer(1)),
        ("default-features", "false".into()),
    ] {
        let mut cargo = base.clone();
        cargo["dependencies"]["byteorder-lite"]
            .as_table_mut()
            .unwrap()
            .insert(key.into(), value);
        attacks.push((format!("dependency option {key}"), cargo));
    }
    for field in ["version", "default-features"] {
        let mut cargo = base.clone();
        cargo["dependencies"]["byteorder-lite"]
            .as_table_mut()
            .unwrap()
            .remove(field);
        attacks.push((format!("missing {field}"), cargo));
    }
    for value in ["=0.1.0".into(), false.into(), toml::Value::Array(vec![])] {
        let mut cargo = base.clone();
        cargo["dependencies"]["byteorder-lite"] = value;
        attacks.push(("non-table dependency".into(), cargo));
    }
    for extra in [
        "[build-dependencies]\nevil='1'",
        "[dev-dependencies]\nevil='1'",
        "[target.'cfg(unix)'.dependencies]\nevil='1'",
        "[target.'cfg(windows)'.build-dependencies]\nevil='1'",
        "[target.wasm32-unknown-unknown.dependencies]\nevil='1'",
        "[patch.crates-io]\nbyteorder-lite={path='../evil'}",
        "[replace]\n'byteorder-lite:0.1.0'={git='https://example.invalid/evil'}",
        "[workspace.dependencies]\nevil='1'",
        "[features]\nescape=['evil']",
        "[profile.release]\nbuild-override={opt-level=3}",
    ] {
        let mut cargo = base.clone();
        let extra: toml::Value = toml::from_str(extra).unwrap();
        cargo
            .as_table_mut()
            .unwrap()
            .extend(extra.as_table().unwrap().clone());
        attacks.push((format!("hidden Cargo section {extra}"), cargo));
    }
    attacks
}

#[test]
fn allowlist_rejects_all_name_source_feature_and_hidden_section_overrides() {
    let (mut cargo, lock) = base();
    assert!(approved(&cargo, &lock).unwrap());
    cargo["dependencies"]["byteorder-lite"]
        .as_table_mut()
        .unwrap()
        .insert("features".into(), toml::Value::Array(vec![]));
    assert!(approved(&cargo, &lock).unwrap());
    for (name, cargo) in cargo_attacks() {
        assert!(approved(&cargo, &lock).is_err(), "accepted {name}");
    }
}

#[test]
fn lockfiles_must_describe_exactly_the_reviewed_graph() {
    let (cargo, lock) = base();
    let lock: toml::Value = toml::from_str(std::str::from_utf8(&lock).unwrap()).unwrap();
    let mut attacks = Vec::new();
    for (field, value) in [
        ("name", "libc"),
        ("version", "0.1.1"),
        ("source", "git+https://example.invalid/evil"),
        ("source", "registry+https://example.invalid/index"),
        ("checksum", ""),
        ("checksum", "invalid"),
    ] {
        let mut bad = lock.clone();
        bad["package"].as_array_mut().unwrap()[1][field] = value.into();
        attacks.push((field.to_owned(), bad));
    }
    for field in ["checksum", "source", "name", "version"] {
        let mut bad = lock.clone();
        bad["package"].as_array_mut().unwrap()[1]
            .as_table_mut()
            .unwrap()
            .remove(field);
        attacks.push((format!("missing {field}"), bad));
    }
    let mut bad = lock.clone();
    bad["package"].as_array_mut().unwrap()[1]
        .as_table_mut()
        .unwrap()
        .insert(
            "dependencies".into(),
            toml::Value::Array(vec!["evil".into()]),
        );
    attacks.push(("transitive dependency".into(), bad));
    let mut bad = lock.clone();
    bad["package"].as_array_mut().unwrap()[0]["dependencies"] = toml::Value::Array(
        vec!["byteorder-lite", "evil"]
            .into_iter()
            .map(Into::into)
            .collect(),
    );
    attacks.push(("root dependency".into(), bad));
    let mut bad = lock.clone();
    let extra = bad["package"].as_array().unwrap()[1].clone();
    bad["package"].as_array_mut().unwrap().push(extra);
    attacks.push(("duplicate package".into(), bad));
    let mut bad = lock.clone();
    bad["package"].as_array_mut().unwrap().remove(1);
    attacks.push(("missing pinned package".into(), bad));
    let mut bad = lock.clone();
    bad.as_table_mut()
        .unwrap()
        .insert("metadata".into(), toml::Value::Table(Default::default()));
    attacks.push(("extra lock section".into(), bad));
    for version in [0, 1, 2, 5] {
        let mut bad = lock.clone();
        bad["version"] = version.into();
        attacks.push((format!("lock version {version}"), bad));
    }
    for (name, lock) in attacks {
        let lock = toml::to_string(&lock).unwrap().into_bytes();
        assert!(approved(&cargo, &lock).is_err(), "accepted {name}");
        let temp = tempfile::tempdir().unwrap();
        let mut files = helper::fixture("examples/good/byteorder-game");
        files.insert("source/Cargo.lock".into(), lock);
        let signed = helper::sign(&files, temp.path());
        let error =
            gamepkg::verify::verify_embedded(&mut Package::open(&signed).unwrap()).unwrap_err();
        assert!(
            !error.to_string().contains("digest mismatch"),
            "wrong failure for {name}: {error}"
        );
    }
    for bytes in [
        &b"\xff"[..],
        &b"version = 4"[..],
        &b"version = 4\npackage = false"[..],
    ] {
        assert!(approved(&cargo, bytes).is_err());
    }
    let mut bad = cargo.clone();
    bad.as_table_mut().unwrap().remove("dependencies");
    assert!(approved(&bad, toml::to_string(&lock).unwrap().as_bytes()).is_err());
    // Order and the supported lockfile version do not change the approved graph.
    let mut reordered = lock.clone();
    reordered["package"].as_array_mut().unwrap().reverse();
    reordered["version"] = 3.into();
    assert!(approved(&cargo, toml::to_string(&reordered).unwrap().as_bytes()).unwrap());
}

#[test]
fn valid_signatures_do_not_authorize_unapproved_crates_at_pack_verify_or_install() {
    for (name, cargo) in cargo_attacks() {
        let temp = tempfile::tempdir().unwrap();
        let mut files = helper::fixture("examples/good/byteorder-game");
        files.insert(
            "source/Cargo.toml".into(),
            toml::to_string(&cargo).unwrap().into_bytes(),
        );
        let source = temp.path().join("source");
        helper::write_fixture(&files, &source);
        let output = temp.path().join("packed.gamepkg");
        assert!(
            gamepkg::pack::pack(&source, &output, &helper::SEED).is_err(),
            "packer accepted {name}"
        );
        assert!(!output.exists() && !output.with_extension("gamepkg.partial").exists());
        let signed = helper::sign(&files, temp.path());
        let mut package = Package::open(&signed).unwrap();
        let envelope: gamepkg::verify::Envelope =
            toml::from_str(std::str::from_utf8(&package.read("signature.toml").unwrap()).unwrap())
                .unwrap();
        let key = ed25519_dalek::SigningKey::from_bytes(&helper::SEED).verifying_key();
        key.verify_strict(
            &gamepkg::verify::tree_hash(&mut package).unwrap(),
            &ed25519_dalek::Signature::from_slice(&hex::decode(envelope.signature).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(
            gamepkg::verify::verify_embedded(&mut package).is_err(),
            "receiver accepted {name}"
        );
        let library = temp.path().join("library");
        let result = Command::new(env!("CARGO_BIN_EXE_gamepkg"))
            .env("GAMEPKG_HOME", &library)
            .env("GAMEPKG_CONFIG", temp.path().join("settings.toml"))
            .arg("install")
            .arg(&signed)
            .arg("--trust-creator")
            .output()
            .unwrap();
        assert!(!result.status.success(), "installer accepted {name}");
        assert_eq!(
            std::fs::read_dir(&library).unwrap().count(),
            0,
            "rejected package left files in library: {name}"
        );
    }
}

#[test]
fn settings_cannot_expand_the_host_owned_crate_catalog() {
    for names in [
        vec!["serde"],
        vec!["byteorder-lite", "serde"],
        vec!["byteorder-lite", "byteorder-lite"],
        vec!["Byteorder-lite"],
        vec!["byteorder_lite"],
    ] {
        let settings = gamepkg::settings::Settings {
            approved_crates: names.into_iter().map(str::to_owned).collect(),
            ..Default::default()
        };
        assert!(settings.validate().is_err());
    }
    let settings = gamepkg::settings::Settings {
        approved_crates: vec![],
        ..Default::default()
    };
    assert!(settings.validate().is_ok());
}
