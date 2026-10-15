//! Scoped asset, selected-ROM and save access. Never accepts a host path.
#![no_std]
unsafe extern "Rust" {
    fn __gamepkg_asset_read(path: &str, offset: u32, output: &mut [u8]) -> Option<u32>;
    fn __gamepkg_rom_count() -> u32;
    fn __gamepkg_rom_size_at(index: u32) -> Option<u32>;
    fn __gamepkg_rom_read_at(index: u32, offset: u32, output: &mut [u8]) -> Option<u32>;
    fn __gamepkg_save_size() -> Option<u32>;
    fn __gamepkg_save_read(offset: u32, output: &mut [u8]) -> Option<u32>;
    fn __gamepkg_save_write(bytes: &[u8]) -> bool;
}
pub fn asset_read(path: &str, offset: u32, output: &mut [u8]) -> Option<u32> {
    unsafe { __gamepkg_asset_read(path, offset, output) }
}
pub fn rom_count() -> u32 { unsafe { __gamepkg_rom_count() } }
pub fn rom_size_at(index: u32) -> Option<u32> { unsafe { __gamepkg_rom_size_at(index) } }
pub fn rom_read_at(index: u32, offset: u32, output: &mut [u8]) -> Option<u32> {
    unsafe { __gamepkg_rom_read_at(index, offset, output) }
}
pub fn save_size() -> Option<u32> { unsafe { __gamepkg_save_size() } }
pub fn save_read(offset: u32, output: &mut [u8]) -> Option<u32> {
    unsafe { __gamepkg_save_read(offset, output) }
}
pub fn save_write(bytes: &[u8]) -> bool { unsafe { __gamepkg_save_write(bytes) } }
