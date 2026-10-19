use gamepkg::manifest::Manifest;
#[test]
fn music_and_effect_reservations_share_the_signed_hard_limit() {
    let original = include_str!("../examples/minimal/manifest.toml");
    let enabled = original.replace("audio = false", "audio = true");
    let valid = enabled.replace(
        "[limits]",
        "[limits]\nmusic_memory_bytes=7340032\neffects_memory_bytes=1048576",
    );
    let manifest = Manifest::parse(valid.as_bytes()).unwrap();
    assert_eq!(manifest.limits.music_memory_bytes, Some(7340032));
    let encoded = toml::to_string(&manifest).unwrap();
    assert_eq!(
        Manifest::parse(encoded.as_bytes())
            .unwrap()
            .limits
            .music_memory_bytes,
        Some(7340032)
    );
    for text in [
        valid.replace("audio = true", "audio = false"),
        valid.replace("music_memory_bytes=7340032", "music_memory_bytes=0"),
        valid.replace("music_memory_bytes=7340032", "music_memory_bytes=65535"),
        valid.replace("music_memory_bytes=7340032", "music_memory_bytes=16777217"),
        valid.replace("music_memory_bytes=7340032", "music_memory_bytes=16711680"),
        valid.replace(
            "hard_memory_bytes = 16777216",
            "hard_memory_bytes = 8388608",
        ),
    ] {
        assert!(Manifest::parse(text.as_bytes()).is_err());
    }
    assert!(
        !toml::to_string(&Manifest::parse(original.as_bytes()).unwrap())
            .unwrap()
            .contains("music_memory_bytes")
    );
}
