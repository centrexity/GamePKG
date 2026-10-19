use gamepkg::manifest::Manifest;
fn source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap()
}
#[test]
fn rom_requirements_are_ordered_bounded_and_unique() {
    let entry = "\n[[roms]]\nid='program'\ntitle='Program ROM'\n";
    let valid = format!("{}{}", source(), entry);
    let manifest = Manifest::parse(valid.as_bytes()).unwrap();
    assert!(manifest.requires_rom);
    assert_eq!(manifest.required_rom_count(), 1);
    assert!(Manifest::parse(format!("{valid}{entry}").as_bytes()).is_err());
    assert!(Manifest::parse(valid.replace("id='program'", "id='../bad'").as_bytes()).is_err());
    assert!(Manifest::parse(valid.replace("title='Program ROM'", "title=''").as_bytes()).is_err());
    let nine = (0..9)
        .map(|i| format!("\n[[roms]]\nid='rom{i}'\ntitle='ROM {i}'\n"))
        .collect::<String>();
    assert!(Manifest::parse(format!("{}{nine}", source()).as_bytes()).is_err());
    assert_eq!(
        Manifest::parse(format!("requires_rom=true\n{}", source()).as_bytes())
            .unwrap()
            .required_rom_count(),
        1
    );
}
