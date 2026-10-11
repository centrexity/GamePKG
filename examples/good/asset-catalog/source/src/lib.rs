pub fn game_tick(_: u32, _: u64) -> u64 {
    let mut bytes = [0; 10];
    let score = if gamepkg::asset_read("levels/first.txt", 0, &mut bytes).is_some() { 1 } else { 0 };
    (score << 32) | (5 << 8) | (10 << 16) | (5 << 24)
}
