use gamepkg::{manifest::Manifest, runtime::Game};
fn manifest() -> Manifest {
    Manifest::parse(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/minimal/manifest.toml"
    )))
    .unwrap()
}
fn module(body: &str) -> Vec<u8> {
    wat::parse_str(body).unwrap()
}
#[test]
fn executes_typed_guest_and_enforces_input_permission() {
    let wasm=module("(module (func (export \"gamepkg_tick\") (param i32 i64) (result i64) local.get 0 i64.extend_i32_u))");
    let mut m = manifest();
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert_eq!(game.step(9).unwrap(), 9);
    m.permissions.input = false;
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert_eq!(game.step(9).unwrap(), 0);
}
#[test]
fn rejects_ambient_imports_and_missing_entrypoints() {
    for import in ["wasi_snapshot_preview1", "env", "network"] {
        let wasm = module(&format!(
            "(module (import \"{import}\" \"fd_write\" (func)))"
        ));
        assert!(Game::from_bytes(&wasm, &manifest())
            .err()
            .unwrap()
            .to_string()
            .contains("cannot import"));
    }
    assert!(Game::from_bytes(&module("(module)"), &manifest()).is_err());
    let mut m = manifest();
    m.requires_rom = true;
    assert!(Game::from_bytes(&module("(module)"), &m)
        .err()
        .unwrap()
        .to_string()
        .contains("ROM delivery"));
}
#[test]
fn fuel_stops_infinite_guest_loops_and_start_functions() {
    let mut m = manifest();
    m.limits.fuel = 1000;
    let wasm=module("(module (func (export \"gamepkg_tick\") (param i32 i64) (result i64) (loop br 0) i64.const 0))");
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert!(game.step(0).is_err());
    let start = module("(module (func $start (loop br 0)) (start $start))");
    assert!(Game::from_bytes(&start, &m).is_err());
}
#[test]
fn memory_limits_reject_initial_and_growing_memory() {
    let mut m = manifest();
    m.limits.hard_memory_bytes = 65536;
    m.limits.recommended_memory_bytes = Some(32768);
    let wasm=module("(module (memory 2) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i64.const 0))");
    assert!(Game::from_bytes(&wasm, &m).is_err());
    let wasm=module("(module (memory 1) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i32.const 1 memory.grow drop i64.const 0))");
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert!(game.step(0).is_err());
}
#[test]
fn epoch_deadline_stops_a_guest_with_excessive_fuel() {
    let mut m = manifest();
    m.limits.fuel = u64::MAX;
    // A host-call loop cannot burn its whole fuel budget before the timer runs
    // on a fast CPU. Still require an Interrupt, not merely any terminating trap.
    let wasm = module(
        r#"(module
        (import "gamepkg" "rom_count" (func $count (result i32)))
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            (loop call $count drop br 0) i64.const 0))"#,
    );
    let settings = gamepkg::settings::Settings {
        fuel_ceiling: 100000000,
        tick_deadline_ms: 10,
        ..Default::default()
    };
    let mut game = Game::with_settings(&wasm, &m, &settings).unwrap();
    let start = std::time::Instant::now();
    let error = game.step(0).unwrap_err();
    assert_eq!(
        error.downcast_ref::<wasmtime::Trap>(),
        Some(&wasmtime::Trap::Interrupt)
    );
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn recommended_memory_does_not_reduce_the_enforced_hard_limit() {
    let mut m = manifest();
    m.limits.hard_memory_bytes = 131072;
    m.limits.recommended_memory_bytes = Some(65536);
    let wasm = module("(module (memory 2) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i64.const 7))");
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert_eq!(game.step(0).unwrap(), 7);
    let too_large=module("(module (memory 3) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i64.const 0))");
    assert!(Game::from_bytes(&too_large, &m).is_err());
}

#[test]
fn user_memory_and_fuel_ceilings_tighten_manifest_limits() {
    let mut m = manifest();
    m.limits.hard_memory_bytes = 2097152;
    let mut settings = gamepkg::settings::Settings {
        memory_ceiling_mib: 1,
        ..Default::default()
    };
    let wasm=module("(module (memory 32) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i64.const 0))");
    assert!(Game::with_settings(&wasm, &m, &settings).is_err());
    let small=module("(module (memory 1) (func (export \"gamepkg_tick\") (param i32 i64) (result i64) i64.const 1))");
    assert!(Game::with_settings(&small, &m, &settings).is_ok());
    settings.fuel_ceiling = 1;
    let wasm=module("(module (func (export \"gamepkg_tick\") (param i32 i64) (result i64) (loop br 0) i64.const 0))");
    let mut game = Game::with_settings(&wasm, &m, &settings).unwrap();
    assert!(game.step(0).is_err());
}

#[test]
fn scoped_asset_import_checks_guest_ranges_and_rejects_unknown_api() {
    let wasm = module(
        r#"(module
        (import "gamepkg" "asset_read" (func $read (param i32 i32 i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
            i32.const -1 i32.const 1 i32.const 0 i32.const 0 i32.const 1 call $read
            i32.const -1 i32.ne if unreachable end
            i32.const 65535 i32.const 1024 i32.const 0 i32.const 0 i32.const 1 call $read
            i32.const -1 i32.ne if unreachable end
            i32.const 0 i32.const 1 i32.const 0 i32.const 2147483647 i32.const 65536 call $read
            i32.const -1 i32.ne if unreachable end
            i64.const 7))"#,
    );
    let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
    assert_eq!(game.step(0).unwrap(), 7);
    let unknown = module(r#"(module (import "gamepkg" "open_file" (func)))"#);
    assert!(Game::from_bytes(&unknown, &manifest()).is_err());
    let wrong = module(r#"(module (import "gamepkg" "asset_read" (func)))"#);
    assert!(Game::from_bytes(&wrong, &manifest()).is_err());
}

#[test]
fn filesystem_and_network_imports_are_denied_before_guest_start() {
    for (namespace, operation) in [
        ("wasi_snapshot_preview1", "path_open"),
        ("wasi_snapshot_preview1", "fd_read"),
        ("wasi_snapshot_preview1", "sock_open"),
        ("wasi_snapshot_preview1", "sock_connect"),
        ("env", "open"),
        ("env", "connect"),
        ("gamepkg", "fs_read"),
        ("gamepkg", "network_connect"),
        ("gamepkg", "path_open"),
    ] {
        let bytes = module(&format!(
            r#"(module
            (import "{namespace}" "{operation}" (func))
            (func $start unreachable) (start $start)
            (func (export "gamepkg_tick") (param i32 i64) (result i64) i64.const 0))"#
        ));
        let error = Game::from_bytes(&bytes, &manifest())
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("cannot import"),
            "{namespace}/{operation}: {error}"
        );
    }
    for resource in ["(memory 1)", "(global i32)", "(table 1 funcref)"] {
        let bytes = module(&format!(
            r#"(module (import "gamepkg" "asset_read" {resource}))"#
        ));
        let error = Game::from_bytes(&bytes, &manifest())
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("cannot import"), "{resource}: {error}");
    }
}
