pub fn next(input: u32, mut x: u32) -> u32 {
    if input & (1 | 256) != 0 { x = (x + 15) % 16; }
    if input & (2 | 512) != 0 { x = (x + 1) % 16; }
    x
}
