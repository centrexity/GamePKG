//! Bounded mono audio output. No microphones, devices, threads or filesystem I/O.
#![no_std]
pub const SAMPLE_RATE: u32 = 48000;
pub const MAX_SAMPLES: usize = 1600;
unsafe extern "Rust" { fn __gamepkg_audio_submit(samples: &[i16]) -> bool; }
pub fn submit(samples: &[i16]) -> bool {
    samples.len() <= MAX_SAMPLES && unsafe { __gamepkg_audio_submit(samples) }
}
/// Decode explicit little-endian signed PCM, rejecting odd or oversized buffers.
pub fn submit_pcm16_le(bytes: &[u8]) -> bool {
    if bytes.len() % 2 != 0 || bytes.len() > MAX_SAMPLES * 2 { return false; }
    let mut samples = [0i16; MAX_SAMPLES];
    let count = bytes.len() / 2;
    let mut index = 0;
    while index < count {
        samples[index] = i16::from_le_bytes([bytes[index * 2], bytes[index * 2 + 1]]);
        index += 1;
    }
    submit(&samples[..count])
}
