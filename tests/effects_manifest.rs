use gamepkg::manifest::Manifest;
#[test]
fn effects_are_opt_in_and_memory_is_bounded_inside_the_signed_hard_limit() {
    let original = include_str!("../examples/minimal/manifest.toml");
    let old = Manifest::parse(original.as_bytes()).unwrap();
    assert_eq!(old.limits.effects_memory_bytes, None);
    assert!(!toml::to_string(&old)
        .unwrap()
        .contains("effects_memory_bytes"));
    for bytes in [0, 1, 65535, 65536, 1048576, 4194304, 4194305, u64::MAX] {
        for allowed in [false, true] {
            let text = original
                .replace(
                    "[limits]",
                    &format!("[limits]\neffects_memory_bytes={bytes}"),
                )
                .replace("audio = false", &format!("audio = {allowed}"));
            let parsed = Manifest::parse(text.as_bytes());
            assert_eq!(
                parsed.is_ok(),
                allowed && (65536..=4194304).contains(&bytes),
                "bytes={bytes}, allowed={allowed}"
            );
            if let Ok(manifest) = parsed {
                let restored =
                    Manifest::parse(toml::to_string(&manifest).unwrap().as_bytes()).unwrap();
                assert_eq!(restored.limits.effects_memory_bytes, Some(bytes));
            }
        }
    }
    let text = original
        .replace("hard_memory_bytes = 16777216", "hard_memory_bytes = 65536")
        .replace(
            "recommended_memory_bytes = 8388608",
            "recommended_memory_bytes = 65536",
        )
        .replace("audio = false", "audio = true")
        .replace("[limits]", "[limits]\neffects_memory_bytes=65536");
    assert!(Manifest::parse(text.as_bytes()).is_err());
}
