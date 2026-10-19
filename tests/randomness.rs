use gamepkg::{manifest::Manifest, runtime::Game};

fn manifest() -> Manifest {
    Manifest::parse(include_bytes!("../examples/minimal/manifest.toml")).unwrap()
}

#[test]
fn core_and_component_random_values_preserve_all_64_bits_without_permissions() {
    for component in [false, true] {
        let mut manifest = manifest();
        manifest.permissions.input = false;
        manifest.permissions.save = false;
        let (namespace, name, entry) = if component {
            manifest.runtime_profile = "component-v1".into();
            ("gamepkg:engine/host@0.1.0", "random-u64", "tick")
        } else {
            ("gamepkg", "random_u64", "gamepkg_tick")
        };
        let bytes = wat::parse_str(format!(
            "(module (import \"{namespace}\" \"{name}\" (func $random (result i64)))
             (memory (export \"memory\") 1)
             (func (export \"{entry}\") (param i32 i64) (result i64) call $random))"
        ))
        .unwrap();
        let bytes = if component {
            gamepkg::hostapi::componentize(bytes).unwrap()
        } else {
            bytes
        };
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        let mut samples = std::collections::BTreeSet::new();
        for _ in 0..128 {
            samples.insert(game.step(0).unwrap());
        }
        assert_eq!(
            samples.len(),
            128,
            "random import returned repeated or truncated values"
        );
        assert!(samples.iter().any(|value| value >> 63 != 0));
        assert!(samples.iter().any(|value| value >> 63 == 0));
    }
}

#[test]
fn random_host_calls_remain_subject_to_the_game_deadline() {
    let bytes = wat::parse_str(
        "(module
      (import \"gamepkg\" \"random_u64\" (func $random (result i64)))
      (func (export \"gamepkg_tick\") (param i32 i64) (result i64)
        (loop call $random drop br 0) i64.const 0))",
    )
    .unwrap();
    let mut game = Game::from_bytes(&bytes, &manifest()).unwrap();
    assert!(game.step(0).is_err());
}
