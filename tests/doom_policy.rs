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
