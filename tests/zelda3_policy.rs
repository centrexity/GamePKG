use gamepkg::manifest::Manifest;
fn source() -> String {
    std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/ports/zelda3/manifest.toml"
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
