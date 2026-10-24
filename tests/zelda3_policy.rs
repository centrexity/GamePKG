use gamepkg::manifest::Manifest;
fn source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/ports/zelda3/manifest.toml"
    ))
    .unwrap()
}
#[test]
fn pinned_engine_requires_rom_permissions_and_memory() {
    let valid = source();
    let manifest = Manifest::parse(valid.as_bytes()).unwrap();
    assert_eq!(manifest.required_rom_count(), 1);
    for bad in [
        valid.replace("zelda3-45a149d8", "arbitrary-native-code"),
        valid.replace("save = true", "save = false"),
        valid.replace("audio = true", "audio = false"),
        valid.replace(
            "hard_memory_bytes = 33554432",
            "hard_memory_bytes = 16777216",
        ),
        valid.replace(
            "entry_world = \"game\"",
            "runtime_profile = \"component-v1\"\nentry_world = \"game\"",
        ),
    ] {
        assert!(Manifest::parse(bad.as_bytes()).is_err());
    }
}
#[test]
fn undeclared_engine_api_cannot_be_called_by_ordinary_packages() {
    let text = source()
        .replace("builtin_engine = \"zelda3-45a149d8\"\n", "")
        .replace(
            "[[roms]]\nid = \"zelda3_usa\"\ntitle = \"Your Zelda: A Link to the Past (USA) ROM\"\n",
            "",
        );
    let manifest = Manifest::parse(text.as_bytes()).unwrap();
    let wasm=wat::parse_str(r#"(module (import "gamepkg" "zelda3_tick" (func $engine (param i32 i64)(result i64)))
        (func (export "gamepkg_tick")(param i32 i64)(result i64) local.get 0 local.get 1 call $engine))"#).unwrap();
    assert!(gamepkg::runtime::Game::from_bytes(&wasm, &manifest).is_err());
}

#[test]
fn signed_video_options_preserve_defaults_and_original_renderer_bounds() {
    let original = Manifest::parse(source().as_bytes()).unwrap();
    assert!(original.zelda_video.is_none());
    assert!(!toml::to_string(&original).unwrap().contains("zelda_video"));
    for (extra, tall, dimensions) in [
        (0, false, (256, 224)),
        (32, false, (320, 224)),
        (96, true, (448, 240)),
    ] {
        let text = format!(
            "{}\n[zelda_video]\nextra_width = {extra}\nextend_height = {tall}\n",
            source()
        );
        let manifest = Manifest::parse(text.as_bytes()).unwrap();
        assert_eq!(manifest.zelda_video.unwrap().dimensions(), dimensions);
        assert_eq!(
            Manifest::parse(toml::to_string(&manifest).unwrap().as_bytes())
                .unwrap()
                .zelda_video,
            manifest.zelda_video
        );
    }
    for invalid in [
        "extra_width = 97",
        "extra_width = -1",
        "extra_width = 4294967295",
        "extra_width = 1.5",
        "extend_height = 1",
        "no_sprite_limits = 1",
        "dim_flashes = 1",
        "skip_intro_on_keypress = 1",
        "extra_width = 0\narbitrary_flag = true",
    ] {
        let text = format!("{}\n[zelda_video]\n{invalid}\n", source());
        assert!(Manifest::parse(text.as_bytes()).is_err(), "{invalid}");
    }
    let enhanced = format!(
        "{}\n[zelda_video]\nno_sprite_limits = true\ndim_flashes = true\nskip_intro_on_keypress = true\n",
        source()
    );
    let enhanced = Manifest::parse(enhanced.as_bytes()).unwrap();
    assert!(enhanced.zelda_video.unwrap().no_sprite_limits);
    assert!(enhanced.zelda_video.unwrap().dim_flashes);
    assert!(enhanced.zelda_video.unwrap().skip_intro_on_keypress);
    let default =
        Manifest::parse(format!("{}\n[zelda_video]\nextra_width = 0\n", source()).as_bytes())
            .unwrap();
    assert!(!toml::to_string(&default)
        .unwrap()
        .contains("no_sprite_limits"));
    assert!(!toml::to_string(&default).unwrap().contains("dim_flashes"));
    let mut all = String::from("\n[zelda_video.enhancements]\n");
    for &(name, _) in gamepkg::manifest::ZeldaEnhancements::CHOICES {
        all.push_str(&format!("{name}=true\n"));
    }
    let enhanced = Manifest::parse(format!("{}{all}", source()).as_bytes()).unwrap();
    assert_eq!(enhanced.zelda_video.unwrap().enhancements.bits(), 0x1fff);
    for invalid in ["invented=true", "item_switch_lr=1"] {
        assert!(Manifest::parse(
            format!("{}\n[zelda_video.enhancements]\n{invalid}\n", source()).as_bytes()
        )
        .is_err());
    }
    let other_source = include_str!("../ports/doom/manifest.toml");
    Manifest::parse(other_source.as_bytes()).expect("non-Zelda control recipe must be valid");
    let wrong_engine = format!("{other_source}\n[zelda_video]\nextra_width = 32\n");
    let error = Manifest::parse(wrong_engine.as_bytes()).unwrap_err();
    assert!(error
        .to_string()
        .contains("Zelda video options require the pinned Zelda recipe"));
}
