#[test]
fn original_ball_texture_matches_the_unchanged_source_and_sdl_reference() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-ball.json")).unwrap();
    let image = image::load_from_memory(include_bytes!("../ports/arkanoo/assets/ball.png"))
        .unwrap()
        .into_rgba8();
    assert_eq!(image.dimensions(), (12, 12));
    let expected: Vec<u8> = fixture["rgba"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();
    assert_eq!(image.as_raw(), &expected);
    assert_eq!(
        gamepkg::verify::digest(&expected),
        fixture["rgba_sha256"].as_str().unwrap()
    );
    assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
    assert_eq!(image.get_pixel(3, 3).0, [255, 255, 255, 255]);
    assert_eq!(image.get_pixel(6, 0).0, [160, 160, 160, 255]);
}

#[test]
fn original_paddle_textures_match_unchanged_source_and_sdl_blending() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-paddle.json")).unwrap();
    for (index, bytes) in [
        include_bytes!("../ports/arkanoo/assets/paddle.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/paddle-long.png").as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        let reference = &fixture["images"][index];
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
        let expected: Vec<u8> = reference["rgba"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(
            image.dimensions(),
            (reference["width"].as_u64().unwrap() as u32, 22)
        );
        assert_eq!(image.as_raw(), &expected);
        assert_eq!(
            gamepkg::verify::digest(&expected),
            reference["rgba_sha256"].as_str().unwrap()
        );
        assert_eq!(image.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(image.get_pixel(70, 11).0, [40, 50, 70, 255]);
    }
}

#[test]
fn original_normal_brick_textures_match_unchanged_source_and_sdl_reference() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-blocks.json")).unwrap();
    for (index, bytes) in [
        include_bytes!("../ports/arkanoo/assets/brick-red.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/brick-orange.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/brick-yellow.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/brick-green.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/brick-blue.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/brick-violet.png").as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        let reference = &fixture["images"][index];
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
        let expected = hex::decode(reference["rgba_hex"].as_str().unwrap()).unwrap();
        assert_eq!(image.dimensions(), (60, 20));
        assert_eq!(image.as_raw(), &expected);
        assert_eq!(
            gamepkg::verify::digest(&expected),
            reference["rgba_sha256"].as_str().unwrap()
        );
        assert!(image.pixels().all(|pixel| pixel.0[3] == 255));
        assert_ne!(image.get_pixel(30, 0), image.get_pixel(30, 19));
    }
}

#[test]
fn original_metal_body_and_unclipped_layers_match_sdl_source_pixels() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arkanoo-metal.json")).unwrap();
    for (index, bytes) in [
        include_bytes!("../ports/arkanoo/assets/metal-body.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/metal-stripes.png").as_slice(),
        include_bytes!("../ports/arkanoo/assets/metal-bevel.png").as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        let layer = &fixture["layers"][index];
        let image = image::load_from_memory(bytes).unwrap().into_rgba8();
        let expected = hex::decode(layer["rgba_hex"].as_str().unwrap()).unwrap();
        assert_eq!(
            image.dimensions(),
            (
                layer["width"].as_u64().unwrap() as u32,
                layer["height"].as_u64().unwrap() as u32
            )
        );
        assert_eq!(image.as_raw(), &expected);
        assert_eq!(
            gamepkg::verify::digest(&expected),
            layer["rgba_sha256"].as_str().unwrap()
        );
        if index == 0 {
            assert!(image.pixels().all(|p| p.0[3] == 255));
            assert!(image.pixels().any(|p| p.0 == [150, 150, 160, 255]));
        } else {
            for y in 0..20 {
                for x in 20..80 {
                    assert_eq!(image.get_pixel(x, y).0, [0, 0, 0, 0]);
                }
            }
        }
        if index == 1 {
            assert_eq!(image.get_pixel(0, 20).0, [255, 255, 255, 30]);
            assert_eq!(image.get_pixel(90, 0).0, [255, 255, 255, 30]);
        }
        if index == 2 {
            assert_eq!(image.get_pixel(80, 0).0, [200, 200, 210, 100]);
            assert_eq!(image.get_pixel(79, 20).0, [0, 0, 0, 150]);
        }
    }
    for (index, composition) in fixture["composites"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            composition["max_channel_delta"].as_u64().unwrap(),
            [2, 2, 2, 2, 1, 1][index]
        );
        assert!(
            composition["differing_bytes"].as_u64().unwrap() > 0,
            "Do not hide SDL line/copy rounding differences"
        );
        assert_ne!(
            composition["original_rgba_sha256"],
            composition["layered_rgba_sha256"]
        );
    }
}
