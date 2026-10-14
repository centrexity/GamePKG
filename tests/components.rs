use gamepkg::{hostapi::componentize, manifest::Manifest, runtime::Game};
fn manifest() -> Manifest {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    ))
    .unwrap();
    Manifest::parse(format!("runtime_profile = \"component-v1\"\n{text}").as_bytes()).unwrap()
}
fn component(source: &str) -> Vec<u8> {
    componentize(wat::parse_str(source).unwrap()).unwrap()
}
#[test]
fn typed_component_draws_and_trapped_ticks_discard_commands() {
    let bytes = component(
        r#"(module
      (import "gamepkg:engine/host@0.1.0" "draw-rect" (func $rect (param f32 f32 f32 f32 i32) (result i32)))
      (memory (export "memory") 1)
      (func (export "tick") (param i32 i64) (result i64)
        f32.const 10 f32.const 20 f32.const 30 f32.const 40 i32.const 0xabcdef12 call $rect drop
        local.get 0 if unreachable end
        local.get 1 i64.const 1 i64.add))"#,
    );
    let mut game = Game::from_bytes(&bytes, &manifest()).unwrap();
    assert_eq!(game.step(0).unwrap(), 1);
    assert_eq!(game.frame().commands.len(), 1);
    assert!(game.step(1).is_err());
    assert!(game.frame().commands.is_empty());
}
#[test]
fn component_input_permissions_and_fuel_are_enforced() {
    let bytes = component(
        r#"(module (memory (export "memory") 1)
      (func (export "tick") (param i32 i64) (result i64) local.get 0 i64.extend_i32_u))"#,
    );
    let mut manifest = manifest();
    manifest.permissions.input = false;
    let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
    assert_eq!(game.step(123).unwrap(), 0);
    let bytes = component(
        r#"(module (memory (export "memory") 1)
      (func (export "tick") (param i32 i64) (result i64) (loop $forever br $forever) i64.const 0))"#,
    );
    assert!(Game::from_bytes(&bytes, &manifest)
        .unwrap()
        .step(0)
        .is_err());
}
#[test]
fn component_memory_and_unknown_interfaces_are_rejected() {
    let bytes = component(
        r#"(module (memory (export "memory") 257)
      (func (export "tick") (param i32 i64) (result i64) i64.const 0))"#,
    );
    assert!(Game::from_bytes(&bytes, &manifest()).is_err());
    let bytes = wat::parse_str(
        r#"(component
      (type $f (func)) (import "wasi:cli/run@0.2.0" (func (type $f))))"#,
    )
    .unwrap();
    assert!(Game::from_bytes(&bytes, &manifest()).is_err());
}

#[test]
fn canonical_abi_rejects_out_of_bounds_guest_strings() {
    let bytes = component(
        r#"(module
      (import "gamepkg:engine/host@0.1.0" "asset-read" (func $asset (param i32 i32 i32 i32 i32)))
      (memory (export "memory") 1)
      (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 32)
      (func (export "tick") (param i32 i64) (result i64)
        i32.const 70000 i32.const 1 i32.const 0 i32.const 1 i32.const 0 call $asset
        i64.const 0))"#,
    );
    let mut game = Game::from_bytes(&bytes, &manifest()).unwrap();
    assert!(game.step(0).is_err());
}

#[test]
fn component_recommended_memory_is_advisory_and_hard_limit_stops_growth() {
    let mut manifest = manifest();
    manifest.limits.hard_memory_bytes = 2 * 65536;
    manifest.limits.recommended_memory_bytes = Some(65536);
    let bytes = component(
        r#"(module (memory (export "memory") 2)
      (func (export "tick") (param i32 i64) (result i64) i64.const 42))"#,
    );
    let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
    assert_eq!(game.step(0).unwrap(), 42);
    let growing = component(
        r#"(module (memory (export "memory") 2)
      (func (export "tick") (param i32 i64) (result i64)
        i32.const 1 memory.grow drop i64.const 0))"#,
    );
    let mut game = Game::from_bytes(&growing, &manifest).unwrap();
    assert!(game.step(0).is_err());
}

#[test]
fn component_user_memory_ceiling_tightens_the_signed_limit() {
    let mut manifest = manifest();
    manifest.limits.hard_memory_bytes = 2 * 1024 * 1024;
    let settings = gamepkg::settings::Settings {
        memory_ceiling_mib: 1,
        ..Default::default()
    };
    let bytes = component(
        r#"(module (memory (export "memory") 32)
      (func (export "tick") (param i32 i64) (result i64) i64.const 42))"#,
    );
    assert!(Game::with_settings(&bytes, &manifest, &settings).is_err());
    let bytes = component(
        r#"(module (memory (export "memory") 16)
      (func (export "tick") (param i32 i64) (result i64) i64.const 42))"#,
    );
    let mut game = Game::with_settings(&bytes, &manifest, &settings).unwrap();
    assert_eq!(game.step(0).unwrap(), 42);
}

#[test]
fn component_sprites_use_the_canonical_pixel_list() {
    let bytes = component(
        r#"(module
      (import "gamepkg:engine/host@0.1.0" "draw-image" (func $image (param f32 f32 f32 f32 i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1) (data (i32.const 0) "\ff\00\00\ff")
      (func (export "tick") (param i32 i64) (result i64)
        f32.const 0 f32.const 0 f32.const 100 f32.const 100
        i32.const 1 i32.const 1 i32.const 0 i32.const 4 call $image
        i64.extend_i32_u))"#,
    );
    let mut game = Game::from_bytes(&bytes, &manifest()).unwrap();
    assert_eq!(game.step(0).unwrap(), 1);
    let gamepkg::drawing::Command::Image { image, .. } = &game.frame().commands[0] else {
        panic!("sprite missing")
    };
    assert_eq!(image.pixels(), &[255, 0, 0, 255]);
}

#[test]
fn component_sprite_lists_are_bounded_before_host_copy() {
    let bytes = component(
        r#"(module
      (import "gamepkg:engine/host@0.1.0" "draw-image" (func $image (param f32 f32 f32 f32 i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 17)
      (func (export "tick") (param i32 i64) (result i64)
        f32.const 0 f32.const 0 f32.const 100 f32.const 100
        i32.const 512 i32.const 512 i32.const 0 i32.const 1048577 call $image
        i64.extend_i32_u))"#,
    );
    let mut game = Game::from_bytes(&bytes, &manifest()).unwrap();
    assert_eq!(game.step(0).unwrap(), 0);
    assert!(game.frame().commands.is_empty());
}

#[test]
fn components_cannot_request_filesystem_or_socket_interfaces() {
    for namespace in [
        "wasi:filesystem/types@0.2.0",
        "wasi:sockets/tcp@0.2.0",
        "wasi:sockets/network@0.2.0",
        "gamepkg:engine/filesystem@0.1.0",
    ] {
        let bytes = wat::parse_str(format!(
            r#"(component
            (type $host (instance))
            (import "{namespace}" (instance (type $host))))"#
        ))
        .unwrap();
        let error = Game::from_bytes(&bytes, &manifest())
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("cannot import"), "{namespace}: {error}");
    }
}
