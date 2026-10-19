use gamepkg::manifest::Manifest;
#[test]
fn shader_entries_validate_paths_names_stages_and_count() {
    let base = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap();
    let good = "\n[[shaders]]\nsource = \"shaders/main.slang\"\nentry = \"fragmentMain\"\nstage = \"fragment\"\n";
    assert_eq!(
        Manifest::parse(format!("{base}{good}").as_bytes())
            .unwrap()
            .shaders
            .len(),
        1
    );
    for invalid in [
        good.replace("shaders/main.slang", "../main.slang"),
        good.replace("shaders/main.slang", "assets/main.slang"),
        good.replace("fragmentMain", "-entry"),
        good.replace("fragmentMain", "1main"),
        good.replace("stage = \"fragment\"", "stage = \"native\""),
    ] {
        assert!(Manifest::parse(format!("{base}{invalid}").as_bytes()).is_err());
    }
    assert!(Manifest::parse(format!("{base}{}", good.repeat(33)).as_bytes()).is_err());
}
