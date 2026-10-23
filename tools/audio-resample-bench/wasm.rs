//! Isolated maintainer comparison module, not a player companion or guest API.
#[allow(dead_code)]
#[path = "main.rs"]
mod benchmark;
#[no_mangle]
pub extern "C" fn compare_audio_resamplers() -> u64 {
    let (cases, frames, checksum) = benchmark::verify();
    assert_eq!(cases, 2592);
    assert_eq!(frames, 5963184);
    checksum
}
