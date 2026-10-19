// A player-supplied data file determines the first target. No ROM is bundled.
pub fn game_tick(input: u32, state: u64) -> u64 {
    if state == 0 {
        let mut first = [0; 2];
        let size = gamepkg::rom_size().unwrap_or(0);
        let read = gamepkg::rom_read(0, &mut first).unwrap_or(0);
        let target_x = if read > 0 { first[0] % 16 } else { 2 };
        let target_y = if read > 1 { first[1] % 16 } else { 2 };
        return 8 | (8 << 8) | ((target_x as u64) << 16) | ((target_y as u64) << 24) | ((size as u64) << 32);
    }
    let mut x = (state as u8).min(15);
    let mut y = ((state >> 8) as u8).min(15);
    if input & 1 != 0 { x = x.saturating_sub(1); }
    if input & 2 != 0 { x = (x + 1).min(15); }
    if input & 4 != 0 { y = y.saturating_sub(1); }
    if input & 8 != 0 { y = (y + 1).min(15); }
    (state & !65535) | x as u64 | ((y as u64) << 8)
}
