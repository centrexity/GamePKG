use gamepkg::{
    drawing::{Command, Frame, MAX_COMMANDS},
    manifest::Manifest,
    runtime::Game,
};
fn manifest() -> Manifest {
    Manifest::parse(
        &std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/minimal/manifest.toml"
        ))
        .unwrap(),
    )
    .unwrap()
}
#[test]
fn validates_coordinates_and_bounds_the_render_queue() {
    let mut frame = Frame::default();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 100000.0] {
        assert!(!frame.rect(value, 0.0, 10.0, 10.0, 0));
        assert!(!frame.circle(0.0, 0.0, value, 0));
    }
    assert!(!frame.rect(0.0, 0.0, -1.0, 10.0, 0));
    for _ in 0..MAX_COMMANDS {
        assert!(frame.circle(10.0, 20.0, 5.0, 0x112233ff));
    }
    assert!(!frame.rect(0.0, 0.0, 10.0, 10.0, 0));
    assert_eq!(frame.commands.len(), MAX_COMMANDS);
    frame.reset();
    assert!(frame.commands.is_empty());
}
#[test]
fn guest_controls_frames_and_trapped_ticks_discard_draws() {
    let wasm = wat::parse_str(
        r#"(module
        (import "gamepkg" "draw_clear" (func $clear (param i32) (result i32)))
        (import "gamepkg" "draw_rect" (func $rect (param f32 f32 f32 f32 i32) (result i32)))
        (func (export "gamepkg_tick") (param i32 i64) (result i64)
          i32.const 0x102030ff call $clear drop
          f32.const 10 f32.const 20 f32.const 30 f32.const 40 i32.const 0xabcdef12 call $rect drop
          local.get 0 if unreachable end
          i64.const 5))"#,
    )
    .unwrap();
    let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
    assert!(game.custom_renderer);
    game.step(0).unwrap();
    assert_eq!(game.frame().background, Some(0x102030ff));
    assert_eq!(
        game.frame().commands,
        vec![Command::Rect {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
            rgba: 0xabcdef12
        }]
    );
    game.step(0).unwrap();
    assert_eq!(game.frame().commands.len(), 1);
    assert!(game.step(1).is_err());
    assert!(game.frame().commands.is_empty());
    assert_eq!(game.frame().background, None);
    assert_eq!(game.state, 5);
}
#[test]
fn draw_calls_consume_guest_fuel() {
    let wasm = wat::parse_str(
        r#"(module
      (import "gamepkg" "draw_clear" (func $clear (param i32) (result i32)))
      (func (export "gamepkg_tick") (param i32 i64) (result i64)
        (loop i32.const 0 call $clear drop br 0) i64.const 0))"#,
    )
    .unwrap();
    let mut m = manifest();
    m.limits.fuel = 1000;
    let mut game = Game::from_bytes(&wasm, &m).unwrap();
    assert!(game.step(0).is_err());
    assert!(game.frame().commands.is_empty());
}

#[test]
fn malicious_guest_cannot_exceed_host_draw_queue() {
    let wasm=wat::parse_str(r#"(module
        (import "gamepkg" "draw_circle" (func $circle (param f32 f32 f32 i32) (result i32)))
        (func (export "gamepkg_tick") (param i32 i64) (result i64) (local $count i32)
            (loop $again
                f32.const 10 f32.const 20 f32.const 5 i32.const -1 call $circle
                i32.const 0 i32.ne if unreachable end
                local.get $count i32.const 1 i32.add local.tee $count i32.const 1024 i32.lt_u br_if $again)
            f32.const 10 f32.const 20 f32.const 5 i32.const -1 call $circle
            i32.const -1 i32.ne if unreachable end
            f32.const nan f32.const 20 f32.const 5 i32.const -1 call $circle
            i32.const -1 i32.ne if unreachable end
            i64.const 1))"#).unwrap();
    let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
    assert_eq!(game.step(0).unwrap(), 1);
    assert_eq!(game.frame().commands.len(), MAX_COMMANDS);
}

#[test]
fn shader_indices_and_command_budget_are_checked() {
    let mut frame = Frame::default();
    assert!(!frame.shader(-1, 2));
    assert!(!frame.shader(2, 2));
    assert!(!frame.shader(0, 0));
    for _ in 0..MAX_COMMANDS {
        assert!(frame.shader(1, 2));
    }
    assert!(!frame.shader(0, 2));
    frame.reset();
    assert!(frame.shader(0, 2));
    assert_eq!(frame.commands, vec![Command::Shader { index: 0 }]);
}
#[test]
fn raw_guest_cannot_draw_unprepared_shader() {
    let wasm = wat::parse_str(
        r#"(module
      (import "gamepkg" "draw_shader" (func $shader (param i32) (result i32)))
      (func (export "gamepkg_tick") (param i32 i64) (result i64)
        i32.const 0 call $shader i64.extend_i32_s))"#,
    )
    .unwrap();
    let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
    assert_eq!(game.step(0).unwrap(), u64::MAX);
    assert!(game.frame().commands.is_empty());
}

#[test]
fn sprite_queue_bounds_memory_and_resets_with_the_frame() {
    use gamepkg::drawing::image::Image;
    let mut frame = Frame::default();
    let pixels = vec![255; 512 * 512 * 4];
    for _ in 0..8 {
        assert!(frame.image(
            0.0,
            0.0,
            512.0,
            512.0,
            Image::new(512, 512, &pixels).unwrap()
        ));
    }
    assert!(!frame.image(0.0, 0.0, 1.0, 1.0, Image::new(1, 1, &[0; 4]).unwrap()));
    frame.reset();
    assert!(!frame.can_image(f32::NAN, 0.0, 1.0, 1.0, 4));
    assert!(!frame.can_image(0.0, 0.0, -1.0, 1.0, 4));
    assert!(frame.image(0.0, 0.0, 1.0, 1.0, Image::new(1, 1, &[0; 4]).unwrap()));
}

#[test]
fn core_sprites_snapshot_pixels_and_reject_invalid_guest_ranges() {
    for (pointer, length, expected) in [(0, 4, 0), (65535, 4, -1), (-1, 4, -1), (0, 5, -1)] {
        let wasm = wat::parse_str(format!(r#"(module
          (import "gamepkg" "draw_image" (func $image (param f32 f32 f32 f32 i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1) (data (i32.const 0) "\ff\00\00\ff")
          (func (export "gamepkg_tick") (param i32 i64) (result i64)
            f32.const 0 f32.const 0 f32.const 100 f32.const 100
            i32.const 1 i32.const 1 i32.const {pointer} i32.const {length} call $image
            i32.const 0 i32.const 0 i32.store
            local.get 0 if unreachable end
            i64.extend_i32_s))"#)).unwrap();
        let mut game = Game::from_bytes(&wasm, &manifest()).unwrap();
        assert!(game.custom_renderer);
        assert_eq!(game.step(0).unwrap(), expected as u64);
        if expected == 0 {
            let Command::Image { image, .. } = &game.frame().commands[0] else {
                panic!("sprite missing")
            };
            assert_eq!(image.pixels(), &[255, 0, 0, 255]);
            assert!(game.step(1).is_err());
            assert!(game.frame().commands.is_empty());
        } else {
            assert!(game.frame().commands.is_empty());
        }
    }
}

#[test]
fn shader_images_share_the_sprite_budget_and_validate_indices() {
    use gamepkg::drawing::image::Image;
    let image = Image::new(512, 512, &vec![255; 512 * 512 * 4]).unwrap();
    let mut frame = Frame::default();
    assert!(!frame.shader_image(-1, 1, image.clone()));
    assert!(!frame.shader_image(1, 1, image.clone()));
    for _ in 0..4 {
        assert!(frame.shader_image(0, 1, image.clone()));
        assert!(frame.image(0.0, 0.0, 1.0, 1.0, image.clone()));
    }
    assert!(!frame.shader_image(0, 1, image.clone()));
    assert!(!frame.image(0.0, 0.0, 1.0, 1.0, image.clone()));
    frame.reset();
    assert!(frame.shader_image(0, 1, image));
}
