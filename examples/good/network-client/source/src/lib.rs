// Start/Enter begins an explicitly permitted HTTPS GET; other frames only poll.
pub fn game_tick(input: u32, state: u64) -> u64 {
    let mut next = state;
    if state == 0 && input & gamepkg::START != 0 {
        next = match gamepkg::https_get("https://example.com/") {
            Ok(handle) => u64::from(handle),
            Err(_) => 2u64 << 32,
        };
    } else if state > 0 && state >> 32 == 0 {
        let mut response = [0u8; 65536];
        next = match gamepkg::network_poll(state as u32, &mut response) {
            Ok(_) => { gamepkg::network_close(state as u32); 1u64 << 32 }
            Err(gamepkg::NetworkError::Pending) => state,
            Err(_) => { gamepkg::network_close(state as u32); 2u64 << 32 }
        };
    }
    let color = if next == 0 { 0x2860c0ff } else if next >> 32 == 1 {
        0x30b870ff
    } else if next >> 32 == 2 { 0xc04040ff } else { 0xe0b030ff };
    gamepkg::draw_clear(0x10171dff);
    gamepkg::draw_rect(200.0, 140.0, 240.0, 200.0, color);
    next
}
