use gamepkg_byteorder::{BigEndian, ByteOrder};
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut out = [0; 4];
    if gamepkg_file::asset_read("hello.txt", 0, &mut out) != Some(4) { return 0; }
    let original = out;
    if gamepkg_file::asset_read("../manifest.toml", 0, &mut out).is_some() || out != original { return 0; }
    if gamepkg_file::rom_count() != 0 || gamepkg_file::rom_size_at(0).is_some() { return 0; }
    if gamepkg_file::rom_read_at(0, 0, &mut out).is_some() || out != original { return 0; }
    if gamepkg_file::save_size().is_some() || gamepkg_file::save_read(0, &mut out).is_some() { return 0; }
    if gamepkg_file::save_write(&out) { return 0; }
    if gamepkg_networking::request("https://example.com", &mut out).is_ok() || out != original { return 0; }
    if !gamepkg_window::clear(0x102030ff) { return 0; }
    if !gamepkg_window::rect(0.0, 0.0, 16.0, 16.0, 0xffffffff) { return 0; }
    if !gamepkg_window::circle(32.0, 32.0, 4.0, 0xffffffff) { return 0; }
    if gamepkg_window::asset_image("missing.png", 0.0, 0.0, 8.0, 8.0) { return 0; }
    let mut x = (state & 15) as u32;
    if gamepkg_input::held(input, gamepkg_input::RIGHT) { x = (x + 1) % 16; }
    if gamepkg_input::pressed(input, gamepkg_input::B) { x = 0; }
    BigEndian::write_u32(&mut out, 42);
    if BigEndian::read_u32(&out) != 42 { return 0; }
    let _ = gamepkg_window::rect(48.0, 48.0, 4.0, 4.0, 0x00ff00ff);
    ((BigEndian::read_u32(&out) as u64) << 32) | x as u64 | (5 << 8) | (10 << 16) | (5 << 24)
}
