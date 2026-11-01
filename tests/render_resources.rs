use gamepkg::{
    drawing::Frame,
    manifest::{Manifest, RenderProfile},
    runtime::Game,
};
use std::sync::Arc;

fn text(large: bool) -> String {
    let text = include_str!("../examples/minimal/manifest.toml").to_owned();
    if large {
        text.replace("[limits]", "[limits]\nrender_profile=\"large-images-v1\"")
            .replace(
                "recommended_memory_bytes = 8388608",
                "recommended_memory_bytes = 268435456",
            )
            .replace(
                "hard_memory_bytes = 16777216",
                "hard_memory_bytes = 268435456",
            )
    } else {
        text
    }
}

#[test]
fn only_the_explicit_signed_profile_can_select_larger_resources() {
    let standard = Manifest::parse(text(false).as_bytes()).unwrap();
    assert_eq!(standard.limits.render_profile, None);
    assert_eq!(standard.limits.web_asset_bytes(), 32 * 1048576);
    assert!(!toml::to_string(&standard)
        .unwrap()
        .contains("render_profile"));
    let large = Manifest::parse(text(true).as_bytes()).unwrap();
    assert_eq!(
        large.limits.render_profile,
        Some(RenderProfile::LargeImagesV1)
    );
    assert_eq!(large.limits.web_asset_bytes(), 128 * 1048576);
    assert!(toml::to_string(&large).unwrap().contains("large-images-v1"));
    for invalid in [
        text(true).replace("large-images-v1", "unlimited"),
        text(true).replace("large-images-v1", "large-images-v2"),
        text(true).replace(
            "hard_memory_bytes = 268435456",
            "hard_memory_bytes = 67108864",
        ),
        text(true).replace(
            "recommended_memory_bytes = 268435456",
            "recommended_memory_bytes = 67108864",
        ),
    ] {
        assert!(Manifest::parse(invalid.as_bytes()).is_err());
    }
    let original = include_str!("../ports/zelda3/manifest.toml");
    Manifest::parse(original.as_bytes()).unwrap();
    let mut builtin: toml::Value = toml::from_str(original).unwrap();
    builtin["limits"].as_table_mut().unwrap().insert(
        "render_profile".into(),
        toml::Value::String("large-images-v1".into()),
    );
    builtin["limits"]["hard_memory_bytes"] = toml::Value::Integer(268435456);
    builtin["limits"]["recommended_memory_bytes"] = toml::Value::Integer(268435456);
    let error = Manifest::parse(toml::to_string(&builtin).unwrap().as_bytes())
        .unwrap_err()
        .to_string();
    assert!(error.contains("source game"), "{error}");
    assert!(large.limits.ensure_render_memory(128 * 1048576).is_err());
    large.limits.ensure_render_memory(256 * 1048576).unwrap();
}

#[test]
fn extended_frames_charge_every_draw_and_clear_does_not_refund() {
    let image =
        Arc::new(gamepkg::drawing::image::Image::new(512, 512, &vec![17; 1048576]).unwrap());
    for (profile, limit) in [(None, 8), (Some(RenderProfile::LargeImagesV1), 32)] {
        let mut frame = Frame::for_profile(profile);
        for _ in 0..2 {
            for index in 0..limit {
                let accepted = match index % 4 {
                    0 => frame.image_arc(0., 0., 1., 1., image.clone(), false),
                    1 => frame.image_arc(0., 0., 1., 1., image.clone(), true),
                    2 => frame.straight_linear_image(0., 0., 1., 1., image.clone()),
                    _ => frame.shader_image(0, 1, image.as_ref().clone()),
                };
                assert!(accepted);
            }
            assert!(!frame.image_arc(0., 0., 1., 1., image.clone(), false));
            assert!(!frame.straight_linear_image(0., 0., 1., 1., image.clone()));
            assert!(!frame.shader_image(0, 1, image.as_ref().clone()));
            frame.clear(0);
            assert!(frame.commands.is_empty());
            assert!(!frame.image_arc(0., 0., 1., 1., image.clone(), false));
            frame.reset();
        }
        assert!(!frame.can_image(f32::NAN, 0., 1., 1., 4));
    }
    assert!(gamepkg::drawing::image::Image::new(513, 1, &vec![0; 513 * 4]).is_none());
}

#[test]
fn core_and_component_hosts_select_the_profile_and_keep_real_memory_ceiling() {
    for component in [false, true] {
        for (large, accepted) in [(false, 8), (true, 32)] {
            let mut manifest = Manifest::parse(text(large).as_bytes()).unwrap();
            let (namespace, name, entry, success) = if component {
                ("gamepkg:engine/host@0.1.0", "draw-image", "tick", 1)
            } else {
                ("gamepkg", "draw_image", "gamepkg_tick", 0)
            };
            let code=std::iter::repeat_n(format!("f32.const 0 f32.const 0 f32.const 1 f32.const 1 i32.const 512 i32.const 512 i32.const 0 i32.const 1048576 call $image i32.const {success} i32.eq if local.get $count i32.const 1 i32.add local.set $count end"),33).collect::<Vec<_>>().join("\n");
            let wasm=wat::parse_str(format!(r#"(module
              (import "{namespace}" "{name}" (func $image (param f32 f32 f32 f32 i32 i32 i32 i32) (result i32)))
              (memory (export "memory") 17)
              (func (export "{entry}") (param i32 i64) (result i64) (local $count i32) {code} local.get $count i64.extend_i32_u))"#)).unwrap();
            let bytes = if component {
                manifest.runtime_profile = "component-v1".into();
                gamepkg::hostapi::componentize(wasm).unwrap()
            } else {
                wasm
            };
            let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
            assert_eq!(game.step(0).unwrap(), accepted);
            assert_eq!(game.frame().commands.len(), accepted as usize);
            let gamepkg::drawing::Command::Image { image: first, .. } = &game.frame().commands[0]
            else {
                panic!("first image")
            };
            let expected_pixels = vec![0; 1048576];
            for command in &game.frame().commands {
                let gamepkg::drawing::Command::Image { image, .. } = command else {
                    panic!("image command")
                };
                assert!(Arc::ptr_eq(first, image));
                assert_eq!(image.pixels(), expected_pixels.as_slice());
            }
            assert_eq!(game.step(0).unwrap(), accepted);
            if large {
                let settings = gamepkg::settings::Settings {
                    memory_ceiling_mib: 128,
                    ..Default::default()
                };
                let error = Game::with_settings(&bytes, &manifest, &settings)
                    .err()
                    .unwrap()
                    .to_string();
                assert!(error.contains("memory ceiling"), "{error}");
            }
        }
    }
}

#[test]
fn adjacent_guest_images_share_only_exact_immutable_snapshots() {
    for component in [false, true] {
        let mut manifest = Manifest::parse(text(false).as_bytes()).unwrap();
        let (namespace, image, linear, entry) = if component {
            manifest.runtime_profile = "component-v1".into();
            (
                "gamepkg:engine/host@0.1.0",
                "draw-image",
                "draw-image-linear",
                "tick",
            )
        } else {
            ("gamepkg", "draw_image", "draw_image_linear", "gamepkg_tick")
        };
        let call = |name, x| {
            format!("f32.const {x} f32.const 0 f32.const 2 f32.const 1 i32.const 2 i32.const 1 i32.const 0 i32.const 8 call ${name} drop")
        };
        let wasm = wat::parse_str(format!(r#"(module
          (import "{namespace}" "{image}" (func $image (param f32 f32 f32 f32 i32 i32 i32 i32) (result i32)))
          (import "{namespace}" "{linear}" (func $linear (param f32 f32 f32 f32 i32 i32 i32 i32) (result i32)))
          (memory (export "memory") 1)
          (func (export "{entry}") (param i32 i64) (result i64)
            {} {} i32.const 7 i32.const 255 i32.store8 {} i64.const 0))"#,
            call("image", 0), call("linear", 1), call("image", 2))).unwrap();
        let bytes = if component {
            gamepkg::hostapi::componentize(wasm).unwrap()
        } else {
            wasm
        };
        let mut game = Game::from_bytes(&bytes, &manifest).unwrap();
        game.step(0).unwrap();
        let commands = &game.frame().commands;
        assert_eq!(commands.len(), 3);
        let gamepkg::drawing::Command::Image {
            image: first,
            x: 0.,
            ..
        } = &commands[0]
        else {
            panic!("first image")
        };
        let gamepkg::drawing::Command::LinearImage {
            image: second,
            x: 1.,
            ..
        } = &commands[1]
        else {
            panic!("linear image")
        };
        let gamepkg::drawing::Command::Image {
            image: changed,
            x: 2.,
            ..
        } = &commands[2]
        else {
            panic!("changed image")
        };
        assert!(Arc::ptr_eq(first, second));
        assert!(!Arc::ptr_eq(first, changed));
        assert_eq!(first.pixels(), &[0; 8]);
        assert_eq!(changed.pixels(), &[0, 0, 0, 0, 0, 0, 0, 255]);
        let retained = first.clone();
        game.step(0).unwrap();
        assert_eq!(retained.pixels(), &[0; 8]);
    }
}

#[test]
fn unsigned_manifest_edits_cannot_grant_the_larger_profile() {
    use std::io::{Read, Write};
    let temp = tempfile::tempdir().unwrap();
    let recipe = temp.path().join("recipe");
    for (name, bytes) in [
        (
            "thumbnail.png",
            include_bytes!("../examples/minimal/thumbnail.png") as &[u8],
        ),
        ("LICENSE", include_bytes!("../examples/minimal/LICENSE")),
        (
            "CREDITS.md",
            include_bytes!("../examples/minimal/CREDITS.md"),
        ),
        (
            "source/Cargo.toml",
            include_bytes!("../examples/minimal/source/Cargo.toml"),
        ),
        (
            "source/Cargo.lock",
            include_bytes!("../examples/minimal/source/Cargo.lock"),
        ),
        (
            "source/src/lib.rs",
            include_bytes!("../examples/minimal/source/src/lib.rs"),
        ),
    ] {
        let output = recipe.join(name);
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        std::fs::write(output, bytes).unwrap();
    }
    let standard = text(false)
        .replace(
            "recommended_memory_bytes = 8388608",
            "recommended_memory_bytes = 268435456",
        )
        .replace(
            "hard_memory_bytes = 16777216",
            "hard_memory_bytes = 268435456",
        );
    std::fs::write(recipe.join("manifest.toml"), standard).unwrap();
    let signed = temp.path().join("signed.gamepkg");
    gamepkg::pack::pack(&recipe, &signed, &[72; 32]).unwrap();
    gamepkg::verify::verify_embedded(&mut gamepkg::archive::Package::open(&signed).unwrap())
        .unwrap();
    let changed = temp.path().join("changed.gamepkg");
    let mut reader = zip::ZipArchive::new(std::fs::File::open(&signed).unwrap()).unwrap();
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&changed).unwrap());
    for index in 0..reader.len() {
        let mut file = reader.by_index(index).unwrap();
        let name = file.name().to_owned();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        if name == "manifest.toml" {
            let mut manifest: toml::Value =
                toml::from_str(std::str::from_utf8(&bytes).unwrap()).unwrap();
            manifest["limits"].as_table_mut().unwrap().insert(
                "render_profile".into(),
                toml::Value::String("large-images-v1".into()),
            );
            bytes = toml::to_string(&manifest).unwrap().into_bytes();
            assert_eq!(
                Manifest::parse(&bytes).unwrap().limits.render_profile,
                Some(RenderProfile::LargeImagesV1)
            );
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
    let error =
        gamepkg::verify::verify_embedded(&mut gamepkg::archive::Package::open(&changed).unwrap())
            .unwrap_err();
    assert!(format!("{error:#}").contains("signature"), "{error:#}");
}
