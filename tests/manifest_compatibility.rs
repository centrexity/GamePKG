use gamepkg::{manifest::Manifest, runtime::Game};
fn source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap()
}
#[test]
fn legacy_defaults_and_explicit_engine_requirements_are_checked() {
    let legacy = Manifest::parse(source().as_bytes()).unwrap();
    assert_eq!(legacy.engine_version, "^0.1.0");
    assert_eq!(legacy.save_schema, 1);
    for requirement in ["=0.1.0", "^0.1", "*", ">=0.1.0, <0.2.0"] {
        let text = format!(
            "engine_version = \"{requirement}\"\nsave_schema = 2\n{}",
            source()
        );
        let manifest = Manifest::parse(text.as_bytes()).unwrap();
        assert_eq!(manifest.save_schema, 2);
    }
    for requirement in ["", "garbage", ">=9.0.0", "<0.1.0"] {
        assert!(Manifest::parse(
            format!("engine_version = \"{requirement}\"\n{}", source()).as_bytes()
        )
        .is_err());
    }
    assert!(Manifest::parse(format!("save_schema = 0\n{}", source()).as_bytes()).is_err());
    let mut future = legacy;
    future.engine_version = "^9.0.0".into();
    assert!(
        Game::from_bytes(&wat::parse_str("(module)").unwrap(), &future)
            .err()
            .unwrap()
            .to_string()
            .contains("requires engine")
    );
}
#[test]
fn creator_identity_is_canonical_and_ids_are_bounded() {
    let key = "AB".repeat(32);
    let manifest =
        Manifest::parse(format!("creator_key = \"{key}\"\n{}", source()).as_bytes()).unwrap();
    assert_eq!(manifest.creator_key, "ab".repeat(32));
    let too_long = source().replace("com.gamepkg.minimal", &"x".repeat(201));
    assert!(Manifest::parse(too_long.as_bytes()).is_err());
}
