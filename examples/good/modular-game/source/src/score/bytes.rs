use byteorder_lite::{BigEndian, ByteOrder};
pub fn read() -> u32 {
    let mut bytes = [0; 4];
    if crate::gamepkg::asset_read("score.bin", 0, &mut bytes) == Some(4) { BigEndian::read_u32(&bytes) } else { 0 }
}
