pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut x = (state & 15) as u64;
    if input & (1 | 256) != 0 { x = (x + 15) % 16; }
    if input & (2 | 512) != 0 { x = (x + 1) % 16; }
    x | (5 << 8) | (10 << 16) | (5 << 24)
}
