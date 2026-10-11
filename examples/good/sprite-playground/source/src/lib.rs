// Four RGBA pixels become a sprite with a transparent lower-right quadrant.
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut x = if state == 0 { 64 } else { state as u32 };
    if input & gamepkg::HELD_LEFT != 0 { x = x.saturating_sub(4); }
    if input & gamepkg::HELD_RIGHT != 0 { x = (x + 4).min(256); }
    let pixels = [255,0,0,255, 0,255,0,255, 0,0,255,255, 255,255,255,0];
    gamepkg::draw_clear(0x10171dff);
    gamepkg::draw_image(x as f32, 64.0, 384.0, 352.0, 2, 2, &pixels);
    x.max(1) as u64
}
