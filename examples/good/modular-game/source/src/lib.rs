mod score;
mod movement;
mod rules { pub mod constants; }
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut points = if state == 0 { score::initial() } else { (state >> 32) as u32 };
    let mut x = movement::next(input, (state & 15) as u32);
    if x == rules::constants::TARGET { points += 1; x = 0; }
    ((points as u64) << 32) | x as u64 | (5 << 8) | (10 << 16) | (5 << 24)
}
