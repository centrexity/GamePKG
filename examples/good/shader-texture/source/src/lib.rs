pub fn game_tick(input: u32, state: u64) -> u64 {
    let pixels = if input & gamepkg::HELD_RIGHT != 0 { [0, 255, 0, 255] } else { [0, 0, 255, 255] };
    let _ = (gamepkg::draw_shader_image(0, 1, 1, &pixels));
    state + 1
}
