// The score counts completed ticks across play sessions.
pub fn game_tick(_: u32, _: u64) -> u64 {
    let mut bytes = [0; 4];
    let _ = gamepkg::save_read(0, &mut bytes);
    let count = u32::from_le_bytes(bytes).saturating_add(1);
    let _ = (gamepkg::save_write(&u32::to_le_bytes(count)));
    (count as u64) << 32 | 8 | (8 << 8) | (2 << 16) | (2 << 24)
}
