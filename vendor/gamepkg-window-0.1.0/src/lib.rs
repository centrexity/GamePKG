//! Draw through the host viewport. No windows, GPU devices or handles are exposed.
#![no_std]
unsafe extern "Rust" {
    fn __gamepkg_draw_clear(rgba: u32) -> bool;
    fn __gamepkg_draw_rect(x: f32, y: f32, width: f32, height: f32, rgba: u32) -> bool;
    fn __gamepkg_draw_circle(x: f32, y: f32, radius: f32, rgba: u32) -> bool;
    fn __gamepkg_draw_asset_image(path: &str, x: f32, y: f32, width: f32, height: f32) -> bool;
}
pub fn clear(rgba: u32) -> bool { unsafe { __gamepkg_draw_clear(rgba) } }
pub fn rect(x: f32, y: f32, width: f32, height: f32, rgba: u32) -> bool {
    unsafe { __gamepkg_draw_rect(x, y, width, height, rgba) }
}
pub fn circle(x: f32, y: f32, radius: f32, rgba: u32) -> bool {
    unsafe { __gamepkg_draw_circle(x, y, radius, rgba) }
}
pub fn asset_image(path: &str, x: f32, y: f32, width: f32, height: f32) -> bool {
    unsafe { __gamepkg_draw_asset_image(path, x, y, width, height) }
}
