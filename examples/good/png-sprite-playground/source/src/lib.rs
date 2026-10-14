// A verified packaged PNG supplies the sprite pixels.
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut x = if state == 0 { 64 } else { state as u32 };
    if input & gamepkg::HELD_LEFT != 0 { x = x.saturating_sub(4); }
    if input & gamepkg::HELD_RIGHT != 0 { x = (x + 4).min(256); }
    gamepkg::draw_clear(0x10171dff);
    let _ = (gamepkg::draw_asset_image("sprite.png", x as f32, 64.0, 384.0, 352.0));
    x.max(1) as u64
}
