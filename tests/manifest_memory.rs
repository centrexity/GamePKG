use gamepkg::manifest::Manifest;
fn source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap()
}
#[test]
fn recommended_memory_is_advisory_and_bounded_by_hard_limit() {
    let m = Manifest::parse(source().as_bytes()).unwrap();
    assert_eq!(m.limits.recommended_memory_bytes(), 8388608);
    assert_eq!(m.limits.hard_memory_bytes, 16777216);
    for value in [0, 16777217] {
        let bad = source().replace(
            "recommended_memory_bytes = 8388608",
            &format!("recommended_memory_bytes = {value}"),
        );
        assert!(Manifest::parse(bad.as_bytes()).is_err());
    }
    for value in [0, 536870913] {
        let bad = source().replace(
            "hard_memory_bytes = 16777216",
            &format!("hard_memory_bytes = {value}"),
        );
        assert!(Manifest::parse(bad.as_bytes()).is_err());
    }
}
#[test]
fn old_memory_field_remains_compatible_and_serializes_canonically() {
    let old = source()
        .replace("recommended_memory_bytes = 8388608\n", "")
        .replace("hard_memory_bytes", "memory_bytes");
    let m = Manifest::parse(old.as_bytes()).unwrap();
    assert_eq!(
        m.limits.recommended_memory_bytes(),
        m.limits.hard_memory_bytes
    );
    let encoded = toml::to_string(&m).unwrap();
    assert!(encoded.contains("hard_memory_bytes = 16777216"));
    let ambiguous = old.replace(
        "memory_bytes = 16777216",
        "memory_bytes = 16777216\nhard_memory_bytes = 16777216",
    );
    assert!(Manifest::parse(ambiguous.as_bytes()).is_err());
}
