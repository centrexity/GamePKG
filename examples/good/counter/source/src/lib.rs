pub fn game_tick(_: u32, state: u64) -> u64 {
    let score = (state >> 32).wrapping_add(1);
    (score << 32) | (5 << 8) | (10 << 16) | (5 << 24)
}
