pub fn game_tick(input: u32, state: u64) -> u64 {
    gamepkg::draw_shader(if input & gamepkg::HELD_RIGHT != 0 { 1 } else { 0 });
    state + 1
}
