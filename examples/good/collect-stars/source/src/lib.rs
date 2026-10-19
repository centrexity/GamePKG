// Grid-v1: arrows/WASD move; collect the yellow target to increase the score.
// Packed state: player x/y, target x/y (8 bits each), score (32 bits).
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut x = state as u8;
    let mut y = (state >> 8) as u8;
    let mut tx = (state >> 16) as u8;
    let mut ty = (state >> 24) as u8;
    let mut score = (state >> 32) as u32;
    if state == 0 { x = 8; y = 8; tx = 2; ty = 2; }
    if input & 1 != 0 { x = x.saturating_sub(1); }
    if input & 2 != 0 { x = (x + 1).min(15); }
    if input & 4 != 0 { y = y.saturating_sub(1); }
    if input & 8 != 0 { y = (y + 1).min(15); }
    if x == tx && y == ty {
        score = score.wrapping_add(1);
        tx = ((score.wrapping_mul(7).wrapping_add(3)) % 16) as u8;
        ty = ((score.wrapping_mul(11).wrapping_add(5)) % 16) as u8;
        if tx == x && ty == y { tx = (tx + 1) % 16; }
    }
    if score >= 1 { gamepkg::achievement_unlock(0); }
    if score >= 10 { gamepkg::achievement_unlock(1); }
    (x as u64) | ((y as u64) << 8) | ((tx as u64) << 16) | ((ty as u64) << 24) | ((score as u64) << 32)
}
