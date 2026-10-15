//! Explicit-endian numbers only. No native-endian or operating-system APIs.
#![no_std]
pub trait ByteOrder {
    fn read_u32(bytes: &[u8]) -> u32;
    fn write_u32(bytes: &mut [u8], value: u32);
}
pub struct BigEndian;
pub struct LittleEndian;
impl ByteOrder for BigEndian {
    fn read_u32(bytes: &[u8]) -> u32 {
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
    fn write_u32(bytes: &mut [u8], value: u32) { bytes[..4].copy_from_slice(&value.to_be_bytes()); }
}
impl ByteOrder for LittleEndian {
    fn read_u32(bytes: &[u8]) -> u32 {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    }
    fn write_u32(bytes: &mut [u8], value: u32) { bytes[..4].copy_from_slice(&value.to_le_bytes()); }
}
