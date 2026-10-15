pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut x = (state & 15) as u32;
    if gamepkg_input::held(input, gamepkg_input::LEFT) { x = (x + 15) % 16; }
    if gamepkg_input::held(input, gamepkg_input::RIGHT) { x = (x + 1) % 16; }
    if gamepkg_input::pressed(input, gamepkg_input::B) { x = 8; }
    x as u64 | (5 << 8) | (10 << 16) | (5 << 24)
}
