//! Trusted host effect voices. Input and music must share a sample rate; mixing
//! precedes output-rate conversion. No game-facing API is exposed here yet.
use crate::memory_budget::{Budget, Lease};
use anyhow::{ensure, Result};
use std::sync::Arc;

const MAX_CLIP_BYTES: usize = 2 * 1024 * 1024;
/// Immutable stereo PCM; shared voices retain one reservation for the recording.
pub struct Clip {
    samples: Box<[i16]>,
    _reservation: Lease,
}
impl Clip {
    pub fn new(samples: &[i16], budget: &Budget) -> Result<Arc<Self>> {
        ensure!(
            !samples.is_empty()
                && samples.len().is_multiple_of(2)
                && samples.len() <= MAX_CLIP_BYTES / 2,
            "invalid effect recording size"
        );
        // Include the ownership header, not just PCM storage, in the charge.
        let bytes = std::mem::size_of_val(samples)
            + std::mem::size_of::<Self>()
            + 2 * std::mem::size_of::<usize>();
        let reservation = budget.reserve(bytes as u64)?;
        Ok(Arc::new(Self {
            samples: samples.into(),
            _reservation: reservation,
        }))
    }
}
struct Voice {
    clip: Arc<Clip>,
    offset: usize,
}
/// Original four channels, allocated from the first available slot. A fifth
/// concurrent request is rejected rather than replacing an existing sound.
pub struct Mixer {
    voices: [Option<Voice>; 4],
    volume: u8,
    paused: bool,
}
impl Default for Mixer {
    fn default() -> Self {
        Self {
            voices: std::array::from_fn(|_| None),
            volume: 128,
            paused: false,
        }
    }
}
/// SDL S16 volume uses integer division toward zero, followed by saturation for
/// each channel independently. Do not sum several voices before clipping.
pub fn mix_sample(destination: i16, source: i16, volume: u8) -> i16 {
    let scaled = i32::from(source) * i32::from(volume.min(128)) / 128;
    (i32::from(destination) + scaled).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}
impl Mixer {
    pub fn play(&mut self, clip: Arc<Clip>) -> Option<usize> {
        let slot = self.voices.iter().position(Option::is_none)?;
        self.voices[slot] = Some(Voice { clip, offset: 0 });
        Some(slot)
    }
    pub fn volume(&mut self, volume: u8) -> bool {
        if volume > 128 {
            return false;
        }
        self.volume = volume;
        true
    }
    pub fn pause(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn stop(&mut self) {
        self.voices.fill_with(|| None);
    }
    pub fn active(&self) -> usize {
        self.voices.iter().filter(|voice| voice.is_some()).count()
    }
    /// Consume one frame from each effect, in slot order, after music. Zero
    /// volume still advances voices; pausing freezes their positions.
    pub fn frame(&mut self, mut music: [i16; 2]) -> [i16; 2] {
        if self.paused {
            return music;
        }
        for slot in &mut self.voices {
            if let Some(voice) = slot {
                for (channel, sample) in music.iter_mut().enumerate() {
                    *sample = mix_sample(
                        *sample,
                        voice.clip.samples[voice.offset + channel],
                        self.volume,
                    );
                }
                voice.offset += 2;
                if voice.offset == voice.clip.samples.len() {
                    *slot = None;
                }
            }
        }
        music
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playing_clips_retain_parent_cache_capacity_after_cache_shutdown() {
        let game = Budget::new(4096);
        let cache = game.partition(1024).unwrap();
        let clip = Clip::new(&[123, -456, 789, -987], &cache).unwrap();
        let mut mixer = Mixer::default();
        assert_eq!(mixer.play(clip.clone()), Some(0));
        assert_eq!(mixer.play(clip.clone()), Some(1));
        let guest = game.reserve(3072).unwrap();
        drop(clip);
        drop(cache);
        assert_eq!(game.used(), 4096);
        assert!(game.reserve(1).is_err());
        assert_eq!(mixer.frame([0; 2]), [246, -912]);
        assert_eq!(game.used(), 4096);
        assert_eq!(mixer.frame([0; 2]), [1578, -1974]);
        assert_eq!(mixer.active(), 0);
        assert_eq!(game.used(), 3072);
        drop(guest);
        assert_eq!(game.used(), 0);
    }
    #[test]
    fn every_source_sample_and_legal_volume_matches_sdl_reference() {
        use sha2::{Digest, Sha256};
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/sdl-s16-mix-reference.json"))
                .unwrap();
        for volume in 0..=128 {
            let mut hash = Sha256::new();
            let mut bytes = [0; 4096];
            for destination in [-32768, -30000, -1, 0, 1, 30000, 32767] {
                for first in (-32768..=32767).step_by(2048) {
                    for (offset, output) in bytes.chunks_exact_mut(2).enumerate() {
                        output.copy_from_slice(
                            &mix_sample(destination, (first + offset as i32) as i16, volume)
                                .to_le_bytes(),
                        );
                    }
                    hash.update(bytes);
                }
            }
            assert_eq!(
                hex::encode(hash.finalize()),
                reference["sha256"][volume as usize].as_str().unwrap(),
                "volume {volume}"
            );
        }
    }
    #[test]
    fn music_then_slot_order_saturates_each_voice_and_reuses_first_free_slot() {
        let budget = Budget::new(4096);
        let positive = Clip::new(&[30000, -30000, 1, -1], &budget).unwrap();
        let negative = Clip::new(&[-30000, 30000], &budget).unwrap();
        let mut mixer = Mixer::default();
        assert_eq!(mixer.play(positive.clone()), Some(0));
        assert_eq!(mixer.play(negative.clone()), Some(1));
        assert_eq!(mixer.frame([30000, -30000]), [2767, -2768]);
        assert_eq!(mixer.active(), 1);
        assert_eq!(mixer.play(negative.clone()), Some(1));
        assert_eq!(mixer.play(negative.clone()), Some(2));
        assert_eq!(mixer.play(negative.clone()), Some(3));
        assert_eq!(mixer.play(negative.clone()), None);
        mixer.stop();
        assert_eq!(mixer.active(), 0);
        drop(positive);
        drop(negative);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn pause_mute_volume_rounding_and_shared_recording_lifetime() {
        let budget = Budget::new(4096);
        let clip = Clip::new(&[-255, 255, -32768, 32767], &budget).unwrap();
        let charged = budget.used();
        let mut mixer = Mixer::default();
        mixer.play(clip.clone());
        mixer.play(clip.clone());
        assert_eq!(budget.used(), charged);
        drop(clip);
        mixer.pause(true);
        assert_eq!(mixer.frame([7, 8]), [7, 8]);
        assert_eq!(mixer.active(), 2);
        assert!(!mixer.volume(129));
        assert!(mixer.volume(64));
        mixer.pause(false);
        assert_eq!(mixer.frame([0, 0]), [-254, 254]);
        assert!(mixer.volume(0));
        assert_eq!(mixer.frame([7, 8]), [7, 8]);
        assert_eq!(mixer.active(), 0);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn invalid_recordings_and_exhausted_budget_release_reservations() {
        let budget = Budget::new(4096);
        for samples in [&[][..], &[1][..]] {
            assert!(Clip::new(samples, &budget).is_err());
        }
        assert!(Clip::new(&vec![0; MAX_CLIP_BYTES / 2 + 2], &budget).is_err());
        assert!(Clip::new(&[0, 0], &Budget::new(3)).is_err());
        assert_eq!(budget.used(), 0);
    }
}
