// Signed as a fixture by service-window. Uses only the generated game API.
fn render() -> Result<(), i32> {
    verify_launch_settings()?;
    verify_memory_layout()?;
    let points = [
        [-1.0f32, -1.0, 0.5, 1.0],
        [3.0, -1.0, 0.5, 1.0],
        [-1.0, 3.0, 0.5, 1.0],
    ];
    let mut bytes = [0u8; 3 * 256];
    for (n, point) in points.into_iter().enumerate() {
        for (i, position) in point.into_iter().enumerate() {
            bytes[n * 256 + i * 4..n * 256 + i * 4 + 4]
                .copy_from_slice(&f32::to_le_bytes(position));
        }
        let diffuse = n * 256 + 3 * 16;
        bytes[diffuse..diffuse + 4].copy_from_slice(&f32::to_le_bytes(1.0));
        bytes[diffuse + 12..diffuse + 16].copy_from_slice(&f32::to_le_bytes(1.0));
        for slot in 0..4 {
            // Nonzero cube direction and finite planar coordinates.
            let coordinate = n * 256 + (9 + slot) * 16;
            bytes[coordinate..coordinate + 4].copy_from_slice(&f32::to_le_bytes(1.0));
            bytes[coordinate + 12..coordinate + 16].copy_from_slice(&f32::to_le_bytes(1.0));
        }
    }
    let vertex = gamepkg::buffer_create(0, &bytes)?;
    let index = gamepkg::buffer_create(1, &[0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0])?;
    let mut words = [0u32; 3268];
    // Fixed vertex path, basic diffuse combiner, no texture sampling.
    words[..16].copy_from_slice(&[
        0x3149564e, 1, 13072, 0, 0, 0, 0x1e19, 6, 0, 0, 0, 0, 0, 8, 0, 0,
    ]);
    let one = f32::to_bits(1.0);
    words[784..788].copy_from_slice(&[one, one, one, 0]);
    words[3264..].copy_from_slice(&[one, one, one, 0]);
    let mut uniform_bytes = [0u8; 13072];
    for (i, word) in words.into_iter().enumerate() {
        uniform_bytes[i * 4..i * 4 + 4].copy_from_slice(&u32::to_le_bytes(word));
    }
    let uniform = gamepkg::buffer_create(2, &uniform_bytes)?;
    let planar = gamepkg::texture_create(1, 1, 1, 1, false)?;
    let cube = gamepkg::texture_create(1, 1, 6, 1, false)?;
    let blue = [0, 0, 255, 255];
    let green = [0, 255, 0, 255];
    gamepkg::texture_write(
        planar,
        gamepkg::TextureRegion {
            mip: 0,
            layer: 0,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
        },
        &blue,
    )?;
    for layer in 0u32..6 {
        gamepkg::texture_write(
            cube,
            gamepkg::TextureRegion {
                mip: 0,
                layer,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            &green,
        )?;
    }
    let target = gamepkg::target_create(64, 64)?;
    for mask in 0u32..16 {
        let mut textures = [gamepkg::TextureBinding {
            texture: planar,
            sampler: 0,
        }; 4];
        for (slot, binding) in textures.iter_mut().enumerate() {
            if mask & (1 << slot) != 0 {
                binding.texture = cube;
            }
        }
        let draw = gamepkg::IndexedDraw {
            pipeline: mask,
            vertex,
            index,
            uniform,
            first_index: 0,
            index_count: 3,
            base_vertex: 0,
            scissor: [0, 0, 64, 64],
            textures: &textures,
        };
        gamepkg::buffer_write(uniform, 28, &u32::to_le_bytes(6))?;
        gamepkg::target_pass(target, 3, 0x000000ff, 1.0, &[draw], None)?;
        verify_pixel(target, [255, 0, 0, 255])?;
        // Select the final combiner. Route each sampled register to RGB D and
        // its alpha to G. Validate every slot with each matching resource kind.
        gamepkg::buffer_write(uniform, 28, &u32::to_le_bytes(2))?;
        for slot in 0u32..4 {
            let cube_slot = mask & (1 << slot) != 0;
            let at = 12768 + u64::from(slot) * 4;
            let mode = if cube_slot { 3 } else { 1 };
            gamepkg::buffer_write(uniform, at, &u32::to_le_bytes(mode))?;
            gamepkg::buffer_write(uniform, 12444, &u32::to_le_bytes(8 + slot))?;
            gamepkg::buffer_write(uniform, 12456, &u32::to_le_bytes(24 + slot))?;
            gamepkg::target_pass(target, 3, 0x000000ff, 1.0, &[draw], None)?;
            verify_pixel(target, if cube_slot { green } else { blue })?;
            gamepkg::buffer_write(uniform, at, &u32::to_le_bytes(0))?;
        }
    }
    gamepkg::target_present(target)
}

fn verify_memory_layout() -> Result<(), i32> {
    let mut windows = [0u8; 64];
    if gamepkg::local_call(1, &[], &mut windows)? != windows.len() {
        return Err(-3);
    }
    for (index, size) in [0x04000000u64, 0x04000000, 0x00100000, 0x00100000]
        .into_iter()
        .enumerate()
    {
        let offset = index * 16 + 8;
        if windows[offset..offset + 8] != u64::to_le_bytes(size) {
            return Err(-3);
        }
    }
    // Commit one page of the scratch window, then round-trip a marker.
    let mut range = [0u8; 28];
    range[..8].copy_from_slice(&windows[48..56]);
    range[16..24].copy_from_slice(&u64::to_le_bytes(4096));
    range[24..28].copy_from_slice(&u32::to_le_bytes(2));
    gamepkg::local_call(2, &range, &mut [])?;
    let mut write = [0u8; 20];
    write[..8].copy_from_slice(&windows[48..56]);
    write[16..].copy_from_slice(b"test");
    gamepkg::local_call(6, &write, &mut [])?;
    let mut value = [0u8; 4];
    range[16..20].copy_from_slice(&u32::to_le_bytes(4));
    if gamepkg::local_call(5, &range[..20], &mut value)? != 4 || value != *b"test" {
        return Err(-3);
    }
    Ok(())
}
