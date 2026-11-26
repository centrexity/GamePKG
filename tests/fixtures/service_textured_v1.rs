// Signed fixture for the host-reviewed textured-v1 native graphics profile.
// Uses only generated GamePKG service APIs; no platform or game assets.
fn textured_vertex(x: f32, y: f32, u: f32, v: f32) -> [u8; 64] {
    let mut words = [0f32; 16];
    words[0] = x;
    words[1] = y;
    words[2] = 0.0;
    words[3] = 1.0;
    words[4] = u;
    words[5] = v;
    words[8] = 1.0;
    words[9] = 1.0;
    words[10] = 1.0;
    words[11] = 1.0;
    let mut bytes = [0u8; 64];
    for (index, word) in words.into_iter().enumerate() {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&f32::to_le_bytes(word));
    }
    bytes
}

fn read_pixel(target: u64, xy: [u32; 2]) -> Result<[u8; 4], i32> {
    let ticket = gamepkg::surface_readback(target, [xy[0], xy[1], 1, 1])?;
    let start = gamepkg::local_performance_sample()?;
    let mut rgba = [0u8; 4];
    loop {
        match gamepkg::readback_poll(ticket, &mut rgba) {
            Ok(4) => return Ok(rgba),
            Err(-2) => {
                let now = gamepkg::local_performance_sample()?;
                if now.counter - start.counter > 2 * start.frequency {
                    return Err(-2);
                }
            }
            _ => return Err(-3),
        }
    }
}

fn verify_textured_pixels(stateful: bool) -> Result<(), i32> {
    let vertices = [
        textured_vertex(-0.5, -0.5, 0.0, 0.0),
        textured_vertex(0.5, -0.5, 0.25, 0.0),
        textured_vertex(0.5, 0.5, 0.25, 0.25),
        textured_vertex(-0.5, 0.5, 0.0, 0.25),
    ]
    .concat();
    let vertex = gamepkg::buffer_create(0, &vertices)?;
    let index = gamepkg::buffer_create(1, &[
        0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0,
    ])?;
    let mut uniform = [0u8; 64];
    for index in 0..4 {
        uniform[index * 4..index * 4 + 4].copy_from_slice(&f32::to_le_bytes(1.0));
    }
    let uniform = gamepkg::buffer_create(2, &uniform)?;
    // First texture column is red in both rows; the other columns carry a
    // different known pattern so an unwired sampler cannot pass the check.
    let texture = gamepkg::texture_create(2, 2, 1, 1, false)?;
    gamepkg::texture_write(
        texture,
        gamepkg::TextureRegion {
            mip: 0,
            layer: 0,
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        },
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255,
        ],
    )?;
    let textures = [gamepkg::TextureBinding {
        texture,
        sampler: 0,
    }];
    let draw = gamepkg::IndexedDraw {
        pipeline: 0,
        vertex,
        index,
        uniform,
        first_index: 0,
        index_count: 6,
        base_vertex: 0,
        scissor: [0, 0, 64, 64],
        textures: &textures,
    };
    let target = gamepkg::target_create(64, 64)?;
    if stateful {
        let state = gamepkg::DrawState {
            pipeline_variant: 0,
            blend_constant: [0.0; 4],
            stencil_reference: 0,
            samplers: &[],
        };
        gamepkg::target_pass_state(target, 3, 0x000000ff, 1.0, &[draw], &[state], None)?;
    } else {
        gamepkg::target_pass(target, 3, 0x000000ff, 1.0, &[draw], None)?;
    }
    if read_pixel(target, [24, 24])? != [255, 0, 0, 255]
        || read_pixel(target, [40, 40])? != [255, 0, 0, 255]
        || read_pixel(target, [4, 4])? != [0, 0, 0, 255]
    {
        return Err(-3);
    }
    Ok(())
}

fn show() -> Result<(), i32> {
    verify_textured_pixels(false)?;
    verify_textured_pixels(true)?;
    let vertices = [
        textured_vertex(-0.5, -0.5, 0.0, 0.0),
        textured_vertex(0.5, -0.5, 0.25, 0.0),
        textured_vertex(0.5, 0.5, 0.25, 0.25),
        textured_vertex(-0.5, 0.5, 0.0, 0.25),
    ]
    .concat();
    let vertex = gamepkg::buffer_create(0, &vertices)?;
    let index = gamepkg::buffer_create(1, &[
        0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0,
    ])?;
    let mut uniform = [0u8; 64];
    for index in 0..4 {
        uniform[index * 4..index * 4 + 4].copy_from_slice(&f32::to_le_bytes(1.0));
    }
    let uniform = gamepkg::buffer_create(2, &uniform)?;
    let texture = gamepkg::texture_create(2, 2, 1, 1, false)?;
    gamepkg::texture_write(
        texture,
        gamepkg::TextureRegion {
            mip: 0,
            layer: 0,
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        },
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255,
        ],
    )?;
    let textures = [gamepkg::TextureBinding {
        texture,
        sampler: 0,
    }];
    let target = gamepkg::target_create(64, 64)?;
    let mut frame = 0u32;
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
                scissor: [0, 0, 64, 64],
                textures: &textures,
            };
            if frame % 2 == 0 {
                gamepkg::target_pass(target, 3, 0x000000ff, 1.0, &[draw], None)?;
            } else {
                let draw_state = gamepkg::DrawState {
                    pipeline_variant: 0,
                    blend_constant: [0.0; 4],
                    stencil_reference: 0,
                    samplers: &[],
                };
                gamepkg::target_pass_state(
                    target,
                    3,
                    0x000000ff,
                    1.0,
                    &[draw],
                    &[draw_state],
                    None,
                )?;
            }
            match gamepkg::target_present(target) {
                Ok(()) | Err(-2) => (),
                Err(error) => return Err(error),
            }
            frame = frame.wrapping_add(1);
        }
        let (now, generation) = gamepkg::monotonic()?;
        gamepkg::wait_until(now.saturating_add(16_000_000), generation)?;
    }
}

pub fn game_main() {
    if show() != Ok(()) {
        return;
    }
}
