use byteorder_lite::{BigEndian, ByteOrder};
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut score = (state >> 32) as u32;
    if state == 0 {
        let mut bytes = [0; 4];
        if gamepkg::asset_read("score.bin", 0, &mut bytes) == Some(4) {
            score = BigEndian::read_u32(&bytes);
        }
    }
    let mut x = (state & 15) as u32;
    if input & (1 | 256) != 0 { x = (x + 15) % 16; }
    if input & (2 | 512) != 0 { x = (x + 1) % 16; }
    if x == 10 { score += 1; x = 0; }
    ((score as u64) << 32) | x as u64 | (5 << 8) | (10 << 16) | (5 << 24)
}
