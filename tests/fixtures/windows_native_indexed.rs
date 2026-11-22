pub fn game_main() {
    let _ = show();
}

fn show() -> Result<(), i32> {
    let positions = [[-0.5f32, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];
    let mut vertices = Vec::new();
    for position in positions {
        for word in [position[0], position[1], 0.5, 1.0, 0.0, 0.0, 1.0, 1.0] {
            vertices.extend(f32::to_le_bytes(word));
        }
    }
    let mut indices = Vec::new();
    for word in [0u32, 1, 2, 0, 2, 3] {
        indices.extend(u32::to_le_bytes(word));
    }
    let mut matrix = Vec::new();
    for i in 0..16 {
        matrix.extend(f32::to_le_bytes(if i % 5 == 0 { 1.0 } else { 0.0 }));
    }
    let vertex = gamepkg::buffer_create(0, &vertices)?;
    let index = gamepkg::buffer_create(1, &indices)?;
    let uniform = gamepkg::buffer_create(2, &matrix)?;
    loop {
        let state = gamepkg::window_state()?;
        if state.closed {
            return Ok(());
        }
        if state.width > 0 && state.height > 0 {
            let draw = gamepkg::IndexedDraw {
                pipeline: 0,
                vertex,
                index,
                uniform,
                first_index: 0,
                index_count: 6,
                base_vertex: 0,
                scissor: [0, 0, state.width, state.height],
                textures: &[],
            };
            match gamepkg::draw_indexed(state.width, state.height, 0x000000ff, 1.0, &[draw]) {
                Ok(()) | Err(-2) => (),
                Err(error) => return Err(error),
            }
        }
        let (now, generation) = gamepkg::monotonic()?;
        gamepkg::wait_until(now.saturating_add(16_000_000), generation)?;
    }
}
