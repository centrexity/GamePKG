// Ordinary checked game source. No host pointers, platform calls or crates.
fn memory_u64(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let part = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}
fn memory_pages(op: u32, id: u64, offset: u64, bytes: u64, mode: u32) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(id));
    request.extend(u64::to_le_bytes(offset));
    request.extend(u64::to_le_bytes(bytes));
    if op != 4 {
        request.extend(u32::to_le_bytes(mode));
    }
    if gamepkg::local_call(op, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn memory_value(id: u64, offset: u64, value: u8) -> Result<(), i32> {
    let mut output = [0u8; 4];
    gamepkg::local_memory_read_segments(&[[id, offset, 4]], &mut output)?;
    if output != [value; 4] {
        return Err(-3);
    }
    Ok(())
}
fn memory_verify() -> Result<(), i32> {
    let mut windows = [0u8; 48];
    if gamepkg::local_call(1, &[], &mut windows)? != 48 {
        return Err(-3);
    }
    let main = memory_u64(&windows, 0)?;
    let contiguous = memory_u64(&windows, 16)?;
    let flash = memory_u64(&windows, 32)?;
    if main == 0
        || contiguous == 0
        || flash == 0
        || main == contiguous
        || main == flash
        || contiguous == flash
        || memory_u64(&windows, 8)? != 67108864
        || memory_u64(&windows, 24)? != 67108864
        || memory_u64(&windows, 40)? != 1048576
    {
        return Err(-3);
    }
    // Every bank starts inaccessible; IDs grant no page access by themselves.
    for id in [main, contiguous, flash] {
        if gamepkg::local_memory_page_states(id, 0, 256)? != [0u8; 256] {
            return Err(-3);
        }
        if gamepkg::local_memory_page_states(id, 0, 257) != Err(-1) {
            return Err(-3);
        }
        if gamepkg::local_memory_fill(id, 0, 1, 0) != Err(-4) {
            return Err(-3);
        }
    }
    for (id, size, byte) in [
        (main, 67108864u64, 17u8),
        (contiguous, 67108864, 34),
        (flash, 1048576, 255),
    ] {
        memory_pages(2, id, 0, size, 2)?;
        if gamepkg::local_memory_page_states(id, 0, 256)? != [3u8; 256] {
            return Err(-3);
        }
        if memory_pages(3, id, 0, 4096, 3) != Err(-1) || memory_pages(2, id, 0, 4096, 3) != Err(-1)
        {
            return Err(-3);
        }
        gamepkg::local_memory_fill(id, 0, size, byte)?;
        memory_value(id, 0, byte)?;
        memory_value(id, size - 4, byte)?;
        if gamepkg::local_memory_fill(id, size, 1, 0) != Err(-1) {
            return Err(-3);
        }
    }
    // Independently mapped banks never alias. Explicit port aliases reuse an
    // opaque ID and backing offset instead of requesting new mappings.
    gamepkg::local_memory_write_segments(&[[main, 0, 4]], &[1, 2, 3, 4])?;
    let mut aliases = [0u8; 8];
    gamepkg::local_memory_read_segments(&[[main, 0, 4], [main, 0, 4]], &mut aliases)?;
    if aliases != [1, 2, 3, 4, 1, 2, 3, 4] {
        return Err(-3);
    }
    memory_value(contiguous, 0, 34)?;
    memory_value(flash, 0, 255)?;
    gamepkg::local_memory_copy(main, 67108860, contiguous, 67108860, 4)?;
    memory_value(contiguous, 67108860, 17)?;
    memory_value(main, 67108860, 17)?;
    memory_pages(3, flash, 0, 1048576, 1)?;
    if gamepkg::local_memory_page_states(flash, 0, 256)? != [2u8; 256] {
        return Err(-3);
    }
    memory_pages(3, main, 4096, 4096, 0)?;
    if gamepkg::local_memory_page_states(main, 4096, 1)? != [1u8]
        || gamepkg::local_memory_fill(main, 4096, 1, 0) != Err(-4)
    {
        return Err(-3);
    }
    memory_pages(3, main, 4096, 4096, 2)?;
    memory_value(main, 4096, 17)?;
    gamepkg::local_memory_copy(flash, 1048572, main, 0, 4)?;
    memory_value(main, 0, 255)?;
    // Every segment/page is checked before writing any earlier valid bank.
    if gamepkg::local_memory_write_segments(&[[main, 0, 4], [flash, 0, 4]], &[0; 8]) != Err(-4)
        || gamepkg::local_memory_copy(main, 0, flash, 0, 4) != Err(-4)
        || gamepkg::local_memory_fill(flash, 0, 1048576, 0) != Err(-4)
    {
        return Err(-3);
    }
    memory_value(main, 0, 255)?;
    memory_value(flash, 0, 255)?;
    memory_pages(4, contiguous, 67104768, 4096, 0)?;
    if gamepkg::local_memory_page_states(contiguous, 67104768, 1)? != [0u8] {
        return Err(-3);
    }
    if gamepkg::local_memory_fill(contiguous, 0, 67108864, 0) != Err(-4) {
        return Err(-3);
    }
    memory_value(contiguous, 0, 34)?;
    memory_value(main, 67108860, 17)?;
    let mut denied = [0x5au8; 8];
    if gamepkg::local_memory_read_segments(&[[main, 0, 4], [contiguous, 67108860, 4]], &mut denied)
        != Err(-4)
        || denied != [0x5a; 8]
    {
        return Err(-3);
    }
    // Neither host addresses nor Xbox device addresses are resource IDs.
    for guessed in [
        0u64, 0x20000000, 0x24000000, 0x28000000, 0xfd000000, 0xfe800000,
    ] {
        if gamepkg::local_memory_fill(guessed, 0, 0, 0) != Err(-4) {
            return Err(-3);
        }
    }
    memory_pages(2, contiguous, 67104768, 4096, 2)?;
    memory_value(contiguous, 67108860, 0)?;
    memory_value(main, 67108860, 17)?;
    Ok(())
}
pub fn game_main() {
    let status = match memory_verify() {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    };
    let _ = gamepkg::diagnostic(1, 0x544b0700, &[status]);
}
