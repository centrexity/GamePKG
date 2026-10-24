use gamepkg::{manifest::Manifest, runtime::Game};
fn source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/ports/doom/manifest.toml"
    ))
    .unwrap()
}
#[test]
fn doom_requires_its_pinned_core_profile_wad_permissions_and_memory() {
    let valid = source();
    let m = Manifest::parse(valid.as_bytes()).unwrap();
    assert_eq!(m.required_rom_count(), 1);
    for bad in [valid.replace("doom-e257a095","doom-arbitrary"),valid.replace("save = true","save = false"),valid.replace("audio = true","audio = false"),valid.replace("hard_memory_bytes = 100663296","hard_memory_bytes = 67108864"),format!("runtime_profile = 'component-v1'\n{valid}"),valid.replace("[[roms]]\nid = \"doom_iwad\"\ntitle = \"Your DOOM.WAD or DOOM2.WAD (original PC IWAD)\"\n","")] {
        assert!(Manifest::parse(bad.as_bytes()).is_err(),"accepted {bad}");
    }
}
#[test]
fn unrelated_packages_cannot_import_either_builtin_tick() {
    let raw=source().replace("builtin_engine = \"doom-e257a095\"\n","").replace("[[roms]]\nid = \"doom_iwad\"\ntitle = \"Your DOOM.WAD or DOOM2.WAD (original PC IWAD)\"\n","");
    let m = Manifest::parse(raw.as_bytes()).unwrap();
    for import in ["doom_tick", "zelda3_tick"] {
        let wasm=wat::parse_str(format!("(module (import \"gamepkg\" \"{import}\" (func (param i32 i64) (result i64))) (memory (export \"memory\") 1 1) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i64.const 0))")).unwrap();
        assert!(Game::from_bytes(&wasm, &m).is_err());
    }
}

#[test]
fn doom_patch_declarations_keep_the_existing_virtual_handle_budget() {
    for patches in 1..=7 {
        let extras = (0..patches)
            .map(|i| format!("[[roms]]\nid = \"patch{i}\"\ntitle = \"Patch {i}\"\n"))
            .collect::<String>();
        let text = source().replace("[limits]", &(extras + "\n[limits]"));
        assert_eq!(
            Manifest::parse(text.as_bytes()).is_ok(),
            patches <= 6,
            "patch count {patches}"
        );
    }
}

#[test]
fn signed_doom_modes_match_declared_patches_and_reject_arbitrary_arguments() {
    let extra = "[[roms]]\nid=\"patch\"\ntitle=\"Patch\"\n";
    let recipe = source().replace("[limits]", &(extra.to_owned() + "[limits]"));
    for mode in ["file", "merge", "nwt_merge", "flats", "sprites", "both"] {
        let text = format!("doom_wad_modes=[\"{mode}\"]\n{recipe}");
        let manifest = Manifest::parse(text.as_bytes()).unwrap();
        let roundtrip = toml::to_string(&manifest).unwrap();
        assert_eq!(
            Manifest::parse(roundtrip.as_bytes())
                .unwrap()
                .doom_wad_modes,
            manifest.doom_wad_modes
        );
    }
    for modes in [
        "[\"merge\",\"file\"]",
        "[\"-merge\"]",
        "[\"../file\"]",
        "[0]",
        "[\"arbitrary\"]",
    ] {
        assert!(Manifest::parse(format!("doom_wad_modes={modes}\n{recipe}").as_bytes()).is_err());
    }
    assert!(
        Manifest::parse(format!("doom_wad_modes=[\"merge\"]\n{}", source()).as_bytes()).is_err()
    );
    let minimal = include_str!("../examples/minimal/manifest.toml");
    assert!(Manifest::parse(format!("doom_wad_modes=[\"file\"]\n{minimal}").as_bytes()).is_err());
    assert!(
        !toml::to_string(&Manifest::parse(source().as_bytes()).unwrap())
            .unwrap()
            .contains("doom_wad_modes")
    );
}
