use gamepkg_input as input;
#[test]
fn portable_buttons_emit_edges_and_held_bits_without_reserved_bits() {
    let buttons = [
        input::LEFT,
        input::RIGHT,
        input::UP,
        input::DOWN,
        input::A,
        input::B,
        input::X,
        input::Y,
        input::L,
        input::R,
        input::START,
        input::SELECT,
    ];
    for button in buttons {
        let first = input::snapshot(button, 0);
        assert!(input::pressed(first, button));
        assert!(input::held(first, button));
        let continuing = input::snapshot(button, button);
        assert!(!input::pressed(continuing, button));
        assert!(input::held(continuing, button));
        assert_eq!(input::snapshot(0, button), 0);
    }
    let simultaneous = input::snapshot(input::LEFT | input::B, 0);
    assert!(input::held(simultaneous, input::LEFT));
    assert!(input::pressed(simultaneous, input::B));
    assert_eq!(
        input::snapshot(u32::MAX, 0),
        input::PRESSED_MASK | input::HELD_MASK
    );
    assert_eq!(input::held_bits(!input::PRESSED_MASK), 0);
    assert!(!input::held(0, u32::MAX));
    assert!(!input::pressed(0, u32::MAX));
}

fn declaration() -> (toml::Value, Vec<u8>) {
    (
        toml::from_str(include_str!(
            "../examples/good/input-game/source/Cargo.toml"
        ))
        .unwrap(),
        include_bytes!("../examples/good/input-game/source/Cargo.lock").to_vec(),
    )
}
#[test]
fn input_crate_is_pinned_without_allowing_device_or_source_overrides() {
    let (cargo, lock) = declaration();
    assert!(gamepkg::allowlist::approved(&cargo, &lock).unwrap());
    for (key, value) in [
        ("version", "*"),
        ("path", "vendor/evil"),
        ("git", "https://evil.example/repo"),
        ("package", "gilrs"),
    ] {
        let mut changed = cargo.clone();
        changed["dependencies"]["gamepkg-input"]
            .as_table_mut()
            .unwrap()
            .insert(key.into(), value.into());
        assert!(gamepkg::allowlist::approved(&changed, &lock).is_err());
    }
    for name in ["gilrs", "egui", "winit", "sdl2", "gamepkg_input"] {
        let mut changed = cargo.clone();
        let entry = changed["dependencies"]
            .as_table_mut()
            .unwrap()
            .remove("gamepkg-input")
            .unwrap();
        changed["dependencies"]
            .as_table_mut()
            .unwrap()
            .insert(name.into(), entry);
        assert!(gamepkg::allowlist::approved(&changed, &lock).is_err());
    }
    let text = std::str::from_utf8(&lock).unwrap();
    for suffix in [
        "\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"",
        "\nchecksum = \"bad\"",
        "\ndependencies = [\"evil\"]",
    ] {
        assert!(
            gamepkg::allowlist::approved(&cargo, format!("{text}{suffix}").as_bytes()).is_err()
        );
    }
    let mut changed = cargo.clone();
    changed["dependencies"]["gamepkg-input"]["default-features"] = true.into();
    assert!(gamepkg::allowlist::approved(&changed, &lock).is_err());
    changed["dependencies"]["gamepkg-input"]["default-features"] = false.into();
    changed["dependencies"]["gamepkg-input"]
        .as_table_mut()
        .unwrap()
        .insert(
            "features".into(),
            toml::Value::Array(vec!["devices".into()]),
        );
    assert!(gamepkg::allowlist::approved(&changed, &lock).is_err());
}
#[test]
fn both_pinned_crates_can_coexist_with_exact_lock_graph() {
    let (mut cargo, lock) = declaration();
    let byteorder: toml::Value = toml::from_str(include_str!(
        "../examples/good/byteorder-game/source/Cargo.toml"
    ))
    .unwrap();
    cargo["dependencies"].as_table_mut().unwrap().insert(
        "gamepkg-byteorder".into(),
        byteorder["dependencies"]["gamepkg-byteorder"].clone(),
    );
    let lock = String::from_utf8(lock).unwrap().replace(
        "dependencies = [\"gamepkg-input\"]",
        "dependencies = [\"gamepkg-byteorder\", \"gamepkg-input\"]",
    );
    let lock = format!("{lock}\n[[package]]\nname = \"gamepkg-byteorder\"\nversion = \"0.1.0\"\n");
    assert!(gamepkg::allowlist::approved(&cargo, lock.as_bytes()).unwrap());
    let entry = gamepkg::allowlist::CATALOG
        .iter()
        .find(|e| e.name == "gamepkg-input")
        .unwrap();
    let temp = tempfile::tempdir().unwrap();
    gamepkg::allowlist::stage_entry(entry, temp.path()).unwrap();
    assert_eq!(
        gamepkg::verify::digest(&std::fs::read(temp.path().join("src/lib.rs")).unwrap()),
        entry.checksum
    );
    syn::parse_file(&std::fs::read_to_string(temp.path().join("src/lib.rs")).unwrap()).unwrap();
}

#[test]
fn only_exact_owned_wrappers_are_catalog_entries_and_source_pins_match() {
    assert_eq!(gamepkg::allowlist::CATALOG.len(), 7);
    for entry in gamepkg::allowlist::CATALOG {
        assert!(entry.name.starts_with("gamepkg-"));
        let directory = tempfile::tempdir().unwrap();
        gamepkg::allowlist::stage_entry(entry, directory.path()).unwrap();
        let source = std::fs::read(directory.path().join("src/lib.rs")).unwrap();
        assert_eq!(gamepkg::verify::digest(&source), entry.checksum);
        syn::parse_file(std::str::from_utf8(&source).unwrap()).unwrap();
        let settings = gamepkg::settings::Settings {
            approved_crates: vec![entry.name.into()],
            ..Default::default()
        };
        assert!(settings.validate().is_ok());
    }
    for name in [
        "byteorder-lite",
        "gilrs",
        "egui",
        "gamepkg-evil",
        "gamepkg-input-extra",
    ] {
        let settings = gamepkg::settings::Settings {
            approved_crates: vec![name.into()],
            ..Default::default()
        };
        assert!(settings.validate().is_err());
    }
    let cargo: toml::Value = toml::from_str(include_str!(
        "../examples/good/wrapper-game/source/Cargo.toml"
    ))
    .unwrap();
    assert!(gamepkg::allowlist::approved(
        &cargo,
        include_bytes!("../examples/good/wrapper-game/source/Cargo.lock")
    )
    .unwrap());
}
