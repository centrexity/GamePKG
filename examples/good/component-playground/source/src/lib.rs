// Move the green square with held arrows/WASD. The guest draws the entire scene.
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut x = if state == 0 { 300 } else { (state & 1023) as u32 };
    let mut y = if state == 0 { 220 } else { ((state >> 10) & 1023) as u32 };
    let frame = ((state >> 20) as u32).wrapping_add(1);
    if input & (1 | gamepkg::HELD_LEFT) != 0 { x = x.saturating_sub(4); }
    if input & (2 | gamepkg::HELD_RIGHT) != 0 { x = (x+4).min(600); }
    if input & (4 | gamepkg::HELD_UP) != 0 { y = y.saturating_sub(4); }
    if input & (8 | gamepkg::HELD_DOWN) != 0 { y = (y+4).min(440); }
    assert!(gamepkg::draw_clear(0x102336ff));
    for row in 0..6 {
        for col in 0..8 {
            assert!(gamepkg::draw_rect((col*80+2) as f32,(row*80+2) as f32,76.0,76.0,0x17324aff));
        }
    }
    assert!(gamepkg::draw_circle((frame % 560 + 40) as f32,80.0,22.0,0xffcf5aff));
    assert!(gamepkg::draw_rect(x as f32,y as f32,40.0,40.0,0x64dcb4ff));
    x as u64 | ((y as u64) << 10) | ((frame as u64) << 20)
}
