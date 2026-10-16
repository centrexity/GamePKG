#[path = "support/adversarial.rs"]
mod helper;
use gamepkg::{archive::Package, dependencies::validate};
use std::process::Command;

fn base() -> (toml::Value, Vec<u8>) {
    let files = helper::fixture("examples/minimal");
    (
        toml::from_str(std::str::from_utf8(&files["source/Cargo.toml"]).unwrap()).unwrap(),
        files["source/Cargo.lock"].clone(),
    )
}
fn attacks() -> Vec<(String, toml::Value)> {
    let (base, _) = base();
    let mut attacks = Vec::new();
    for name in [
        "serde",
        "libc",
        "std",
        "tokio",
        "gilrs",
        "egui",
        "byteorder-lite",
        "gamepkg-input",
        "gamepkg-file",
        "gamepkg-window",
        "gamepkg-networking",
        "gamepkg-byteorder",
        "gamepkg-shader",
        "gamepkg-audio",
        "gamepkg-evil",
        "gamepkg_input",
        "Gamepkg-input",
        "gamepkg-input ",
        "gamepkg-input\u{200b}",
    ] {
        for declaration in [
            "\"1\"",
            "false",
            "[]",
            "{version=\"=0.1.0\",default-features=false}",
            "{version=\"*\"}",
            "{path=\"../../outside\"}",
            "{git=\"https://example.invalid/repo\"}",
            "{package=\"gamepkg-input\",version=\"=0.1.0\"}",
            "{version=\"=0.1.0\",optional=true}",
        ] {
            let value: toml::Value =
                toml::from_str(&format!("[dependencies]\n'{name}'={declaration}")).unwrap();
            let mut cargo = base.clone();
            cargo
                .as_table_mut()
                .unwrap()
                .insert("dependencies".into(), value["dependencies"].clone());
            attacks.push((format!("{name:?} {declaration}"), cargo));
        }
    }
    for extra in [
        "[build-dependencies]\nevil='1'",
        "[dev-dependencies]\nevil='1'",
        "[target.'cfg(unix)'.dependencies]\nevil='1'",
        "[target.'cfg(windows)'.build-dependencies]\nevil='1'",
        "[workspace.dependencies]\nevil='1'",
        "[patch.crates-io]\nevil={path='../evil'}",
        "[replace]\n'evil:0.1.0'={git='https://example.invalid/evil'}",
        "[features]\ndefault=['evil']",
        "[target.wasm32-unknown-unknown.dependencies]\nevil='1'",
        "[workspace]\nmembers=['evil']",
    ] {
        let mut cargo = base.clone();
        let addition: toml::Value = toml::from_str(extra).unwrap();
        cargo
            .as_table_mut()
            .unwrap()
            .extend(addition.as_table().unwrap().clone());
        attacks.push((extra.into(), cargo));
    }
    attacks
}
#[test]
fn no_guest_crates_even_renamed_optional_pinned_or_previously_approved() {
    let (cargo, lock) = base();
    validate(&cargo, &lock).unwrap();
    let mut empty = cargo.clone();
    empty.as_table_mut().unwrap().insert(
        "dependencies".into(),
        toml::Value::Table(Default::default()),
    );
    validate(&empty, &lock).unwrap();
    for (name, cargo) in attacks() {
        assert!(validate(&cargo, &lock).is_err(), "accepted {name}");
    }
}
#[test]
fn only_the_exact_root_package_may_appear_in_the_lockfile() {
    let (cargo, lock) = base();
    let lock: toml::Value = toml::from_str(std::str::from_utf8(&lock).unwrap()).unwrap();
    for field in ["dependencies", "source", "checksum", "replace", "metadata"] {
        let mut bad = lock.clone();
        bad["package"].as_array_mut().unwrap()[0]
            .as_table_mut()
            .unwrap()
            .insert(field.into(), "evil".into());
        assert!(validate(&cargo, toml::to_string(&bad).unwrap().as_bytes()).is_err());
    }
    for name in ["gamepkg-input", "serde"] {
        let mut bad = lock.clone();
        let entry: toml::Value =
            toml::from_str(&format!("name={name:?}\nversion='0.1.0'")).unwrap();
        bad["package"].as_array_mut().unwrap().push(entry);
        assert!(validate(&cargo, toml::to_string(&bad).unwrap().as_bytes()).is_err());
    }
    for version in [0, 1, 2, 5] {
        let mut bad = lock.clone();
        bad["version"] = version.into();
        assert!(validate(&cargo, toml::to_string(&bad).unwrap().as_bytes()).is_err());
    }
    let mut old = lock.clone();
    old["version"] = 3.into();
    validate(&cargo, toml::to_string(&old).unwrap().as_bytes()).unwrap();
    for bytes in [
        &b"\xff"[..],
        &b"version=4"[..],
        &b"version=4\npackage=false"[..],
    ] {
        assert!(validate(&cargo, bytes).is_err());
    }
}
#[test]
fn signed_dependencies_are_rejected_before_pack_verify_install_or_build() {
    for (name, cargo) in attacks() {
        let temp = tempfile::tempdir().unwrap();
        let mut files = helper::fixture("examples/minimal");
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
        assert!(!output.exists());
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
        let config = temp.path().join("settings.toml");
        std::fs::write(
            &config,
            "approved_crates=['gamepkg-input','serde','gamepkg-evil']\n",
        )
        .unwrap();
        let library = temp.path().join("library");
        let result = Command::new(env!("CARGO_BIN_EXE_gamepkg"))
            .env("GAMEPKG_CONFIG", &config)
            .env("GAMEPKG_HOME", &library)
            .args(["install", signed.to_str().unwrap(), "--trust-creator"])
            .output()
            .unwrap();
        assert!(!result.status.success(), "installer accepted {name}");
        if library.exists() {
            assert_eq!(std::fs::read_dir(&library).unwrap().count(), 0);
        }
        std::fs::create_dir_all(&library).unwrap();
        std::fs::copy(&signed, library.join("com.gamepkg.minimal.gamepkg")).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_gamepkg"))
            .env("GAMEPKG_CONFIG", &config)
            .env("GAMEPKG_HOME", &library)
            .args(["build", "com.gamepkg.minimal"])
            .output()
            .unwrap();
        assert!(!result.status.success(), "builder accepted {name}");
        assert!(
            !library.join("builds").exists(),
            "rejected dependency reached a build stage"
        );
    }
}
