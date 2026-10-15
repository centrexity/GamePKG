//! Signed Slang sources and bounded textures through the host renderer.
#![no_std]
unsafe extern "Rust" {
    fn __gamepkg_shader_index(source: &str, entry: &str) -> Option<u32>;
    fn __gamepkg_shader_draw(index: u32) -> bool;
    fn __gamepkg_shader_rgba(index: u32, width: u32, height: u32, pixels: &[u8]) -> bool;
    fn __gamepkg_shader_texture(index: u32, path: &str) -> bool;
}
pub struct Shader { index: u32 }
impl Shader {
    /// Resolve a signed manifest entry. Compilation is performed by the host on Build.
    pub fn from_slang(source: &str, entry: &str) -> Option<Self> {
        unsafe { __gamepkg_shader_index(source, entry) }.map(|index| Self { index })
    }
    pub fn draw(&self) -> bool { unsafe { __gamepkg_shader_draw(self.index) } }
    /// Submit exactly width*height*4 RGBA bytes, at most 512x512 pixels.
    pub fn draw_rgba(&self, width: u32, height: u32, pixels: &[u8]) -> bool {
        if width == 0 || height == 0 || width > 512 || height > 512
            || pixels.len() != width as usize * height as usize * 4 { return false; }
        unsafe { __gamepkg_shader_rgba(self.index, width, height, pixels) }
    }
    /// Decode a bounded PNG from the package's virtual assets; no host paths.
    pub fn draw_texture(&self, path: &str) -> bool {
        if path.len() > 1024 { return false; }
        unsafe { __gamepkg_shader_texture(self.index, path) }
    }
}
