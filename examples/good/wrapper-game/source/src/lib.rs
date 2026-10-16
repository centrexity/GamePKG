pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut out = [0; 4];
    if gamepkg::asset_read("hello.txt", 0, &mut out) != Some(4) { return 0; }
    let original = out;
    if gamepkg::asset_read("../manifest.toml", 0, &mut out).is_some() || out != original { return 0; }
    if gamepkg::rom_count() != 0 || gamepkg::rom_size_at(0).is_some() { return 0; }
    if gamepkg::rom_read_at(0, 0, &mut out).is_some() || out != original { return 0; }
    if gamepkg::save_size().is_some() || gamepkg::save_read(0, &mut out).is_some() { return 0; }
    if gamepkg::save_write(&out) { return 0; }
    if gamepkg::network_request("https://example.com", &mut out).is_ok() || out != original { return 0; }
    if !gamepkg::draw_clear(0x102030ff) { return 0; }
    if !gamepkg::draw_rect(0.0, 0.0, 16.0, 16.0, 0xffffffff) { return 0; }
    if !gamepkg::draw_circle(32.0, 32.0, 4.0, 0xffffffff) { return 0; }
    if gamepkg::draw_asset_image("missing.png", 0.0, 0.0, 8.0, 8.0) { return 0; }
    let mut x = (state & 15) as u32;
    if gamepkg::held(input, gamepkg::RIGHT) { x = (x + 1) % 16; }
    if gamepkg::pressed(input, gamepkg::B) { x = 0; }
    out.copy_from_slice(&u32::to_be_bytes(42));
    if u32::from_be_bytes([out[0], out[1], out[2], out[3]]) != 42 { return 0; }
    let _ = gamepkg::draw_rect(48.0, 48.0, 4.0, 4.0, 0x00ff00ff);
    ((u32::from_be_bytes([out[0], out[1], out[2], out[3]]) as u64) << 32) | x as u64 | (5 << 8) | (10 << 16) | (5 << 24)
}
