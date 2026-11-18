use super::Result;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub focused: bool,
    pub closed: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCommand {
    Fullscreen(bool),
    CursorVisible(bool),
    CursorLocked(bool),
    Resize(u32, u32),
    Close,
}
/// Immutable copied RGBA8 texture. No backend texture, device or surface leaks.
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
pub struct Draw {
    pub image: Option<Arc<Image>>,
    pub solid: Option<u32>,
    pub rect: [f32; 4],
    pub linear: bool,
    pub clip: Option<[f32; 4]>,
    pub circle: bool,
    pub straight: bool,
    /// Index in the host's verified compiled shader set, not arbitrary source.
    pub shader: Option<u32>,
}
pub struct Frame {
    pub clear: u32,
    pub draws: Vec<Draw>,
}

/// Implemented only by audited GamePKG backend code. Never by package code.
/// Calls return Busy for backpressure; they must not silently discard audio or
/// present only a prefix of a frame. No callbacks execute inside a global lock.
pub trait Backend {
    fn capture(&mut self, _operation: u32, _request: &[u8], _capacity: usize) -> Result<Vec<u8>> {
        Err(super::Error::Unsupported)
    }
    fn revoke_capture(&mut self) {}
    fn audio_streams(&self) -> Option<super::audio_streams::Mixer> {
        None
    }
    /// Audited host GPU service; the default cannot accept native 3D commands.
    fn graphics(&mut self, _operation: u32, _request: &[u8], _capacity: usize) -> Result<Vec<u8>> {
        Err(super::Error::Unsupported)
    }
    fn tick_begin(&mut self) -> Result<()> {
        Ok(())
    }
    fn tick_finish(&mut self) -> Result<()> {
        Ok(())
    }
    fn tick_audio(&mut self, _operation: u32, _name: &str, _value: u32) -> bool {
        false
    }
    fn tick_homepage(&mut self, _url: String) {}
    fn texture_limit(&self) -> u32 {
        8192
    }
    fn window(&mut self, command: WindowCommand) -> Result<()>;
    fn shader_count(&self) -> u32 {
        0
    }
    fn present(&mut self, frame: Frame) -> Result<()>;
    /// Interleaved signed PCM16 at 48 kHz; channels is 1 or 2. Output only.
    fn audio(&mut self, samples: &[i16], channels: u32) -> Result<()>;
    /// The selected shared device identity, never a physical XInput slot.
    fn rumble(&mut self, device: u64, low: u16, high: u16, milliseconds: u32) -> Result<()>;
}
