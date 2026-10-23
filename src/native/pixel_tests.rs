include!("pixels.rs.txt");
#[test]
fn little_and_big_words_produce_identical_rgba_for_every_rgb_channel_pair() {
    for a in 0u32..=255 {
        for b in 0u32..=255 {
            for (word, expected) in [
                (
                    0x7f000000 | (a << 16) | (b << 8) | (a ^ b),
                    [a as u8, b as u8, (a ^ b) as u8, 255],
                ),
                (
                    0xff000000 | (a << 16) | ((a ^ b) << 8) | b,
                    [a as u8, (a ^ b) as u8, b as u8, 255],
                ),
                (
                    ((a ^ b) << 16) | (a << 8) | b,
                    [(a ^ b) as u8, a as u8, b as u8, 255],
                ),
            ] {
                let mut little = std::hint::black_box(word.to_le_bytes());
                let mut big = std::hint::black_box(word.to_be_bytes());
                __gamepkg_engine_rgba(&mut little, true);
                __gamepkg_engine_rgba(&mut big, false);
                assert_eq!(little, expected);
                assert_eq!(big, expected);
            }
        }
    }
}
#[test]
fn native_color_and_fixed_little_endian_stream_contracts_are_distinct() {
    assert!(!__gamepkg_engine_little_endian(0, false));
    assert!(__gamepkg_engine_little_endian(0, true));
    assert!(__gamepkg_engine_little_endian(1, false));
    assert!(__gamepkg_engine_little_endian(1, true));

    let mut native_big = [0x7f, 0x12, 0x34, 0x56];
    let mut doom_stream = [0x56, 0x34, 0x12, 0];
    __gamepkg_engine_rgba(&mut native_big, false);
    __gamepkg_engine_rgba(&mut doom_stream, true);
    assert_eq!(native_big, [0x12, 0x34, 0x56, 255]);
    assert_eq!(native_big, doom_stream);
    let mut ending_big = [0xff, 0xa8, 0, 0];
    __gamepkg_engine_rgba(&mut ending_big, false);
    assert_eq!(ending_big, [0xa8, 0, 0, 255]);
    let mut empty = [];
    __gamepkg_engine_rgba(&mut empty, false);
}
#[test]
#[should_panic(expected = "invalid engine pixel length")]
fn partial_color_words_are_rejected() {
    __gamepkg_engine_rgba(&mut [1, 2, 3], true);
}

#[test]
#[should_panic(expected = "unsupported engine pixel format")]
fn unknown_pixel_formats_are_rejected() {
    __gamepkg_engine_little_endian(2, false);
}
