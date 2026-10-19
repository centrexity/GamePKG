use gamepkg::{catalog::Catalog, manifest::Manifest};
fn base() -> &'static str {
    include_str!("../examples/minimal/manifest.toml")
}
fn parse(value: toml::Value) -> anyhow::Result<Manifest> {
    Manifest::parse(toml::to_string(&value)?.as_bytes())
}
#[test]
fn old_packages_keep_their_id_and_do_not_emit_empty_metadata() {
    let manifest = Manifest::parse(base().as_bytes()).unwrap();
    assert_eq!(manifest.display_title(), manifest.id);
    let value = toml::Value::try_from(&manifest).unwrap();
    for key in ["title", "description", "author", "homepage", "genres"] {
        assert!(value.get(key).is_none());
    }
}
#[test]
fn metadata_roundtrips_and_rejects_unsafe_or_excessive_values() {
    let mut value: toml::Value = toml::from_str(base()).unwrap();
    for (key, text) in [
        ("title", "Puzzle Garden"),
        (
            "description",
            "A puzzle adventure.\nPlay with a controller or keyboard.",
        ),
        ("author", "Example Studio"),
        ("homepage", "https://example.com/game"),
    ] {
        value
            .as_table_mut()
            .unwrap()
            .insert(key.into(), text.into());
    }
    value.as_table_mut().unwrap().insert(
        "genres".into(),
        toml::Value::try_from(vec!["Puzzle", "Adventure"]).unwrap(),
    );
    let manifest = parse(value.clone()).unwrap();
    assert_eq!(manifest.display_title(), "Puzzle Garden");
    assert!(manifest.description.unwrap().contains('\n'));
    for (key, bad) in [
        ("title", "".into()),
        ("title", "x".repeat(257)),
        ("description", " ".into()),
        ("description", "x".repeat(4097)),
        ("description", "bad\u{1b}[2J".into()),
        ("description", "bad\u{202e}text".into()),
        ("author", "x".repeat(129)),
        ("homepage", "javascript:alert(1)".into()),
        ("homepage", "https://user:secret@example.com/".into()),
    ] {
        let mut changed = value.clone();
        changed[key] = toml::Value::String(bad);
        assert!(parse(changed).is_err(), "accepted invalid {key}");
    }
    for genres in [
        vec!["Action"; 9],
        vec!["Action", "action"],
        vec![""],
        vec!["bad\nlabel"],
    ] {
        let mut changed = value.clone();
        changed["genres"] = toml::Value::try_from(genres).unwrap();
        assert!(parse(changed).is_err());
    }
}
#[test]
fn catalog_descriptions_persist_offline_as_untrusted_plain_text() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("input.gamepkglist");
    let source="format_version=1\n[[games]]\nid='com.example.game'\ntitle='Example'\ndescription='A puzzle <script>literal text</script>'\ndownload_url='https://example.invalid/game.gamepkg'\n";
    std::fs::write(&input, source).unwrap();
    let library = root.path().join("library");
    gamepkg::catalog::import(&library, &input).unwrap();
    assert_eq!(
        gamepkg::catalog::entries(&library).unwrap()[0]
            .description
            .as_deref(),
        Some("A puzzle <script>literal text</script>")
    );
    assert!(!library.join("com.example.game.gamepkg").exists());
    assert!(Catalog::parse(
        source
            .replace("A puzzle <script>literal text</script>", &"x".repeat(4097))
            .as_bytes()
    )
    .is_err());
    assert!(Catalog::parse(
        source
            .replace(
                "description='A puzzle <script>literal text</script>'",
                "description=''"
            )
            .as_bytes()
    )
    .is_err());
}
#[test]
fn signed_description_cannot_be_changed_without_the_creator_key() {
    use std::io::{Read, Write};
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("game");
    std::fs::create_dir(&source).unwrap();
    fn copy(from: &std::path::Path, to: &std::path::Path) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    copy(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/minimal"),
        &source,
    );
    std::fs::write(
        source.join("manifest.toml"),
        format!(
            "title='Puzzle Garden'\ndescription='Original description'\n{}",
            base()
        ),
    )
    .unwrap();
    let archive = temp.path().join("original.gamepkg");
    gamepkg::pack::pack(&source, &archive, &[37; 32]).unwrap();
    let mut original = gamepkg::archive::Package::open(&archive).unwrap();
    assert_eq!(
        gamepkg::verify::verify_embedded(&mut original)
            .unwrap()
            .description
            .as_deref(),
        Some("Original description")
    );
    let mut reader = zip::ZipArchive::new(std::fs::File::open(archive).unwrap()).unwrap();
    let changed = temp.path().join("changed.gamepkg");
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&changed).unwrap());
    for index in 0..reader.len() {
        let mut entry = reader.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name == "manifest.toml" {
            bytes = String::from_utf8(bytes)
                .unwrap()
                .replace("Original description", "Tampered description")
                .into_bytes();
        }
        writer
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap();
    assert!(gamepkg::verify::verify_embedded(
        &mut gamepkg::archive::Package::open(&changed).unwrap()
    )
    .is_err());
}
