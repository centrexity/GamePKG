fn run() -> Result<(), i32> {
    let mut layout = [0u8; 64];
    if gamepkg::local_call(1, &[], &mut layout)? != 64 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        layout[48], layout[49], layout[50], layout[51], layout[52], layout[53], layout[54],
        layout[55],
    ]);
    let size = u64::from_le_bytes([
        layout[56], layout[57], layout[58], layout[59], layout[60], layout[61], layout[62],
        layout[63],
    ]);
    if window == 0 || size != 1048576 {
        return Err(-3);
    }
    let commit = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(0),
        &u64::to_le_bytes(65536),
        &u32::to_le_bytes(2),
    ]
    .concat();
    gamepkg::local_call(2, &commit, &mut [])?;
    let mut expected = std::vec::Vec::new();
    expected.resize(12312, 0u8);
    let mut replacement = std::vec::Vec::new();
    replacement.resize(12312, 0x5au8);
    if !gamepkg::local_memory_compare_exchange(window, 0, &expected, &replacement)? {
        return Err(-3);
    }
    if gamepkg::local_memory_compare_exchange(window, 0, &expected, &replacement)? {
        return Err(-3);
    }
    let mut observed = std::vec::Vec::new();
    observed.resize(12312, 0u8);
    let read = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(0),
        &u32::to_le_bytes(12312),
    ]
    .concat();
    if gamepkg::local_call(5, &read, &mut observed)? != observed.len() || observed != replacement {
        return Err(-3);
    }
    let mut records = std::vec::Vec::new();
    let mut record_bytes = std::vec::Vec::new();
    record_bytes.resize(4096, 0u8);
    for _ in 0..16u32 {
        records.push(gamepkg::local_record_create(&record_bytes)?);
    }
    if gamepkg::local_record_create(&[0u8; 1]) != Err(-2) {
        return Err(-3);
    }
    for record in records {
        gamepkg::local_record_release(record)?;
    }
    let protect = [
        u64::to_le_bytes(window).as_slice(),
        &u64::to_le_bytes(4096),
        &u64::to_le_bytes(4096),
        &u32::to_le_bytes(1),
    ]
    .concat();
    gamepkg::local_call(3, &protect, &mut [])?;
    if gamepkg::local_memory_compare_exchange(window, 0, &replacement, &expected) != Err(-4) {
        return Err(-3);
    }
    if gamepkg::local_call(5, &read, &mut observed)? != observed.len() || observed != replacement {
        return Err(-3);
    }
    gamepkg::diagnostic(1, 0x53435241, &[4, size, 12312, 65536])?;
    Ok(())
}
pub fn game_main() {
    let code = match run() {
        Ok(()) => 0u64,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x534352ff, &[code]);
}
