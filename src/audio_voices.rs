//! Trusted host effect voices. Input and music must share a sample rate; mixing
//! precedes output-rate conversion. No game-facing API is exposed here yet.
use crate::memory_budget::{Budget, Lease};
use anyhow::{ensure, Result};
use std::{io::Read, sync::Arc};

const MAX_CLIP_BYTES: usize = 2 * 1024 * 1024;
/// Immutable stereo PCM; shared voices retain one reservation for the recording.
pub struct Clip {
    samples: Box<[i16]>,
    _reservation: Lease,
}
impl Clip {
    fn reservation(samples: usize, budget: &Budget) -> Result<Lease> {
        budget.reserve(
            (samples * 2 + std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>()) as u64,
        )
    }
    pub fn new(samples: &[i16], budget: &Budget) -> Result<Arc<Self>> {
        ensure!(
            !samples.is_empty()
                && samples.len().is_multiple_of(2)
                && samples.len() <= MAX_CLIP_BYTES / 2,
            "invalid effect recording size"
        );
        // Include the ownership header, not just PCM storage, in the charge.
        let reservation = Self::reservation(samples.len(), budget)?;
        Ok(Arc::new(Self {
            samples: samples.into(),
            _reservation: reservation,
        }))
    }
    /// Load canonical 44.1 kHz stereo S16 WAV for the original-rate host mixer.
    /// Verification, audio permission and the caller's shared budget are required.
    /// The complete ZIP CRC is checked before a clip can escape this call.
    pub fn from_verified_asset(
        package: &mut crate::archive::Package,
        path: &str,
        audio_allowed: bool,
        budget: &Budget,
    ) -> Result<Arc<Self>> {
        ensure!(audio_allowed, "effects require audio permission");
        ensure!(
            package
                .asset_compression(path)?
                .is_none_or(|method| method == zip::CompressionMethod::Stored),
            "compressed effects require decompressor memory accounting"
        );
        package.with_asset_reader(path, (MAX_CLIP_BYTES + 44) as u64, |input, length| {
            Self::read_original_wav(input, length, budget)
        })
    }
    fn read_original_wav(
        input: &mut dyn Read,
        length: usize,
        budget: &Budget,
    ) -> Result<Arc<Self>> {
        ensure!(
            (48..=MAX_CLIP_BYTES + 44).contains(&length),
            "invalid effect WAV length"
        );
        // Reserve the fixed read staging too, including the header. No full
        // encoded copy or uncharged decoding allocation is retained.
        let _staging = budget.reserve(4096 + 44)?;
        let mut header = [0; 44];
        input.read_exact(&mut header)?;
        let word = |at| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        ensure!(
            &header[..4] == b"RIFF"
                && &header[8..16] == b"WAVEfmt "
                && word(4) as usize == length - 8
                && word(16) == 16
                && header[20..24] == [1, 0, 2, 0]
                && word(24) == 44100
                && word(28) == 176400
                && header[32..36] == [4, 0, 16, 0]
                && &header[36..40] == b"data"
                && word(40) as usize == length - 44
                && (length - 44).is_multiple_of(4),
            "effects require canonical 44100 Hz stereo S16 WAV"
        );
        let count = (length - 44) / 2;
        let reservation = Self::reservation(count, budget)?;
        let mut samples = vec![0; count].into_boxed_slice();
        let mut block = [0; 4096];
        for output in samples.chunks_mut(2048) {
            let bytes = &mut block[..output.len() * 2];
            input.read_exact(bytes)?;
            for (sample, bytes) in output.iter_mut().zip(bytes.as_chunks::<2>().0.iter()) {
                *sample = i16::from_le_bytes(*bytes);
            }
        }
        Ok(Arc::new(Self {
            samples,
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
    fn wav(samples: &[i16]) -> Vec<u8> {
        let mut bytes = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0\x01\0\x02\0\x44\xac\0\0\x10\xb1\x02\0\x04\0\x10\0data\0\0\0\0".to_vec();
        let size = samples.len() as u32 * 2;
        bytes[4..8].copy_from_slice(&(size + 36).to_le_bytes());
        bytes[40..44].copy_from_slice(&size.to_le_bytes());
        for sample in samples {
            bytes.extend(sample.to_le_bytes());
        }
        bytes
    }
    #[test]
    fn original_wav_streams_little_endian_stereo_across_read_boundaries() {
        struct SmallReads(std::io::Cursor<Vec<u8>>);
        impl Read for SmallReads {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                let count = output.len().min(3);
                self.0.read(&mut output[..count])
            }
        }
        let expected = (0..4098).map(|i| (i * 7919) as i16).collect::<Vec<_>>();
        let bytes = wav(&expected);
        let budget = Budget::new(16384);
        let clip = Clip::read_original_wav(
            &mut SmallReads(std::io::Cursor::new(bytes.clone())),
            bytes.len(),
            &budget,
        )
        .unwrap();
        assert_eq!(&*clip.samples, expected);
        assert_eq!(budget.used(), clip._reservation.bytes());
        let mut mixer = Mixer::default();
        mixer.play(clip.clone());
        drop(clip);
        for frame in expected.as_chunks::<2>().0 {
            assert_eq!(mixer.frame([0; 2]), *frame);
        }
        assert_eq!(mixer.active(), 0);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn malformed_original_wavs_and_read_failures_release_every_charge() {
        let valid = wav(&[i16::MIN, i16::MAX, 1, -1]);
        let budget = Budget::new(8192);
        for at in [0, 4, 8, 12, 16, 20, 22, 24, 28, 32, 34, 36, 40] {
            let mut bytes = valid.clone();
            bytes[at] ^= 1;
            assert!(
                Clip::read_original_wav(&mut &bytes[..], bytes.len(), &budget).is_err(),
                "header byte {at}"
            );
            assert_eq!(budget.used(), 0);
        }
        for size in 0..valid.len() {
            assert!(Clip::read_original_wav(&mut &valid[..size], valid.len(), &budget).is_err());
            assert_eq!(budget.used(), 0);
        }
        for bytes in [wav(&[]), wav(&[1]), wav(&[1, 2, 3])] {
            assert!(Clip::read_original_wav(&mut &bytes[..], bytes.len(), &budget).is_err());
            assert_eq!(budget.used(), 0);
        }
        for limit in [1, 4139, 4140, 4141] {
            let denied = Budget::new(limit);
            assert!(Clip::read_original_wav(&mut &valid[..], valid.len(), &denied).is_err());
            assert_eq!(denied.used(), 0);
        }
        // Length rejection happens before any attempted read/allocation.
        assert!(
            Clip::read_original_wav(&mut std::io::empty(), MAX_CLIP_BYTES + 48, &budget).is_err()
        );
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn original_wav_exact_clip_limit_and_denial_before_payload_reads() {
        let samples = vec![12345; MAX_CLIP_BYTES / 2];
        let bytes = wav(&samples);
        let budget = Budget::new((MAX_CLIP_BYTES + 8192) as u64);
        let clip = Clip::read_original_wav(&mut &bytes[..], bytes.len(), &budget).unwrap();
        assert_eq!(&*clip.samples, samples);
        drop(clip);
        assert_eq!(budget.used(), 0);
        let denied = Budget::new(4140);
        let mut input = std::io::Cursor::new(&bytes);
        assert!(Clip::read_original_wav(&mut input, bytes.len(), &denied).is_err());
        assert_eq!(input.position(), 44);
        assert_eq!(denied.used(), 0);
        // One additional stereo frame exceeds the unchanged recording bound.
        let mut input = std::io::Cursor::new(&bytes);
        assert!(Clip::read_original_wav(&mut input, bytes.len() + 4, &budget).is_err());
        assert_eq!(input.position(), 0);
        assert_eq!(budget.used(), 0);
    }
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
                    for (offset, output) in bytes.as_chunks_mut::<2>().0.iter_mut().enumerate() {
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
