pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut x = (state & 15) as u32;
    if gamepkg::held(input, gamepkg::LEFT) { x = (x + 15) % 16; }
    if gamepkg::held(input, gamepkg::RIGHT) { x = (x + 1) % 16; }
    if gamepkg::pressed(input, gamepkg::B) { x = 8; }
    x as u64 | (5 << 8) | (10 << 16) | (5 << 24)
}
