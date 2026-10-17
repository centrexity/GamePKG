use gamepkg::manifest::Manifest;
fn cube() -> toml::Value {
    toml::from_str(include_str!("../ports/cube2/manifest.toml")).unwrap()
}
fn accepts(value: &toml::Value) -> bool {
    Manifest::parse(toml::to_string(value).unwrap().as_bytes()).is_ok()
}
#[test]
fn user_selected_servers_require_a_signed_reviewed_selector_and_no_static_grants() {
    let valid = cube();
    assert!(accepts(&valid));
    for mode in ["none", "https", "all"] {
        let mut m = valid.clone();
        m["permissions"]["network"] = mode.into();
        assert!(!accepts(&m));
    }
    let mut m = valid.clone();
    m["permissions"]
        .as_table_mut()
        .unwrap()
        .insert("network_allow_local".into(), true.into());
    assert!(!accepts(&m));
    for field in ["network_hosts", "network_listen"] {
        let mut m = valid.clone();
        m["permissions"].as_table_mut().unwrap().insert(
            field.into(),
            toml::Value::Array(vec!["127.0.0.1:28785".into()]),
        );
        assert!(!accepts(&m));
    }
    let mut m = valid.clone();
    m["game_data"]["user_selected_servers"] = false.into();
    assert!(!accepts(&m));
    m["permissions"]["network"] = "none".into();
    assert!(accepts(&m));
    let mut m = valid;
    m["builtin_engine"] = "doom-rust".into();
    assert!(!accepts(&m));
}
#[test]
fn only_reviewed_native_cube_profile_can_request_folders_and_large_memory() {
    let valid = cube();
    assert!(accepts(&valid));
    for field in ["input", "audio", "save"] {
        let mut m = valid.clone();
        m["permissions"][field] = false.into();
        assert!(!accepts(&m));
    }
    for size in [0, 512 * 1048576, 4294967297u64] {
        let mut m = valid.clone();
        m["limits"]["hard_memory_bytes"] = (size as i64).into();
        assert!(!accepts(&m));
    }
    let mut m = valid.clone();
    m.as_table_mut().unwrap().remove("game_data");
    assert!(!accepts(&m));
    let mut m = valid.clone();
    m.as_table_mut()
        .unwrap()
        .insert("runtime_profile".into(), "component-v1".into());
    assert!(!accepts(&m));
    let mut m = valid.clone();
    m["builtin_engine"] = "unreviewed-engine".into();
    assert!(!accepts(&m));
    let mut m = valid.clone();
    m.as_table_mut().unwrap().remove("builtin_engine");
    assert!(!accepts(&m));
    let mut ordinary: toml::Value =
        toml::from_str(include_str!("../examples/minimal/manifest.toml")).unwrap();
    ordinary["limits"]
        .as_table_mut()
        .unwrap()
        .remove("memory_bytes");
    ordinary["limits"]
        .as_table_mut()
        .unwrap()
        .insert("hard_memory_bytes".into(), 2147483648i64.into());
    assert!(!accepts(&ordinary));
}
#[test]
fn cube_metadata_cannot_expand_wasm_memory_permissions() {
    let manifest = Manifest::parse(include_bytes!("../ports/cube2/manifest.toml")).unwrap();
    let wasm=wat::parse_str("(module (memory (export \"memory\") 1 1) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i64.const 0))").unwrap();
    assert!(gamepkg::runtime::Game::from_bytes(&wasm, &manifest).is_err());
}
#[test]
fn data_folder_locations_are_scoped_and_failed_changes_preserve_records() {
    let library = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    std::fs::create_dir(data.path().join("data")).unwrap();
    std::fs::create_dir(data.path().join("packages")).unwrap();
    let mut manifest = Manifest::parse(include_bytes!("../ports/cube2/manifest.toml")).unwrap();
    gamepkg::game_data::remember(library.path(), &manifest, data.path()).unwrap();
    assert_eq!(
        gamepkg::game_data::load(library.path(), &manifest)
            .unwrap()
            .unwrap(),
        data.path()
    );
    manifest.creator_key = "11".repeat(32);
    assert!(gamepkg::game_data::load(library.path(), &manifest)
        .unwrap()
        .is_none());
    let before = std::fs::read(library.path().join("game-data-selections.json")).unwrap();
    assert!(gamepkg::game_data::remember(library.path(), &manifest, library.path()).is_err());
    assert_eq!(
        std::fs::read(library.path().join("game-data-selections.json")).unwrap(),
        before
    );
}
