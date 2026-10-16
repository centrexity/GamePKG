pub fn read() -> u32 {
    let mut bytes = [0; 4];
    if crate::gamepkg::asset_read("score.bin", 0, &mut bytes) == Some(4) { u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) } else { 0 }
}
