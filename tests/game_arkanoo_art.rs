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
