//! Host-only original-rate music/effect mix, followed by one output conversion.
use crate::{audio_resample::Resampler, audio_voices, memory_budget};
use anyhow::{ensure, Result};
const MAX_FRAMES: usize = 800;
pub struct Stream {
    converter: Resampler,
    effects: audio_voices::Mixer,
    input: [[f32; 2]; MAX_FRAMES],
    output: [[f32; 2]; MAX_FRAMES],
    finished: bool,
    _reservation: memory_budget::Lease,
}
impl Stream {
    pub fn new(budget: &memory_budget::Budget) -> Result<Self> {
        let reservation = budget.reserve(std::mem::size_of::<Self>() as u64)?;
        Ok(Self {
            converter: Resampler::new(44100, 48000).map_err(anyhow::Error::msg)?,
            effects: audio_voices::Mixer::default(),
            input: [[0.0; 2]; MAX_FRAMES],
            output: [[0.0; 2]; MAX_FRAMES],
            finished: false,
            _reservation: reservation,
        })
    }
    pub fn effects(&mut self) -> &mut audio_voices::Mixer {
        &mut self.effects
    }
    pub fn input_capacity(&self) -> usize {
        self.converter.input_capacity()
    }
    /// Music must already have its original integer volume applied. Effect
    /// recordings must be 44.1 kHz stereo. Commands affect subsequently appended
    /// frames, never audio already buffered by the output converter.
    pub fn append(&mut self, music: &[[i16; 2]]) -> Result<()> {
        ensure!(!self.finished, "combined audio stream is finished");
        ensure!(
            music.len() <= self.input_capacity(),
            "combined audio input is full"
        );
        for (input, music) in self.input.iter_mut().zip(music) {
            *input = self
                .effects
                .frame(*music)
                .map(|sample| f32::from(sample) / 32768.0);
        }
        self.converter
            .append(&self.input[..music.len()])
            .map_err(anyhow::Error::msg)
    }
    pub fn render(&mut self, output: &mut [[i16; 2]]) -> Result<usize> {
        ensure!(
            output.len() <= MAX_FRAMES,
            "combined audio output is too large"
        );
        let n = self
            .converter
            .render(&mut self.output[..output.len()])
            .map_err(anyhow::Error::msg)?;
        for (output, input) in output.iter_mut().zip(&self.output[..n]) {
            *output = input.map(|sample| {
                (sample * 32768.0)
                    .round_ties_even()
                    .clamp(-32768.0, 32767.0) as i16
            });
        }
        Ok(n)
    }
    pub fn finish(&mut self) {
        self.finished = true;
        self.converter.finish();
    }
}
/// Device-clock adapter. Music and effects continue independently of game ticks.
/// Silence fills genuine music underruns; effects continue at the original rate.
pub struct Playback {
    stream: Stream,
    music: Option<crate::music::Output>,
    input: [[i16; 2]; 64],
    _reservation: memory_budget::Lease,
}
impl Playback {
    pub fn new(music: crate::music::Output, budget: &memory_budget::Budget) -> Result<Self> {
        ensure!(
            music.sample_rate() == 44100,
            "combined playback requires original-rate music"
        );
        Self::with_source(Some(music), budget)
    }
    /// Effects-only device-clock source; no decoder or music ring is needed.
    pub fn effects_only(budget: &memory_budget::Budget) -> Result<Self> {
        Self::with_source(None, budget)
    }
    fn with_source(
        music: Option<crate::music::Output>,
        budget: &memory_budget::Budget,
    ) -> Result<Self> {
        let reservation =
            budget.reserve((std::mem::size_of::<Self>() - std::mem::size_of::<Stream>()) as u64)?;
        Ok(Self {
            stream: Stream::new(budget)?,
            music,
            input: [[0; 2]; 64],
            _reservation: reservation,
        })
    }
    pub fn effects(&mut self) -> &mut audio_voices::Mixer {
        self.stream.effects()
    }
    pub fn frame(&mut self) -> Result<[i16; 2]> {
        let mut output = [[0; 2]; 1];
        // Each iteration either outputs a frame or appends input. SDL's bounded
        // filter lookahead is smaller than this fixed maximum refill count.
        for _ in 0..32 {
            if self.stream.render(&mut output)? == 1 {
                return Ok(output[0]);
            }
            let count = self.input.len().min(self.stream.input_capacity());
            ensure!(count > 0, "combined playback cannot progress");
            self.input[..count].fill([0; 2]);
            if let Some(music) = &self.music {
                music.read(&mut self.input[..count]);
            }
            self.stream.append(&self.input[..count])?;
        }
        anyhow::bail!("combined playback exceeded its refill bound")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    fn music(frame: usize) -> [i16; 2] {
        std::array::from_fn(|channel| {
            (((frame * 7919 + channel * 31337) % 65536) as i32 - 32768) as i16
        })
    }
    fn clip(slot: usize) -> Vec<i16> {
        (0..[1025, 2051, 4096, 777][slot])
            .flat_map(|frame| {
                (0..2).map(move |channel| {
                    (((frame * (slot * 1543 + 197) + channel * 12345 + slot * 313) % 65536) as i32
                        - 32768) as i16
                })
            })
            .collect()
    }
    #[test]
    fn original_order_then_conversion_matches_sdl_for_every_chunk_schedule() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/sdl-s16-mix-reference.json"))
                .unwrap();
        for record in reference["combined_streams"].as_array().unwrap() {
            for (put, get) in [(1, 1), (113, 799), (800, 71)] {
                let budget = memory_budget::Budget::new(128 * 1024);
                let mut stream = Stream::new(&budget).unwrap();
                assert!(stream
                    .effects()
                    .volume(record["volume"].as_u64().unwrap() as u8));
                for slot in 0..4 {
                    assert_eq!(
                        stream
                            .effects()
                            .play(audio_voices::Clip::new(&clip(slot), &budget).unwrap()),
                        Some(slot)
                    );
                }
                let mut hash = Sha256::new();
                let mut at = 0;
                let mut frames = 0;
                let mut output = [[0; 2]; 800];
                loop {
                    let count = stream.render(&mut output[..get]).unwrap();
                    for frame in &output[..count] {
                        for sample in frame {
                            hash.update(sample.to_le_bytes());
                        }
                    }
                    frames += count;
                    if count > 0 {
                        continue;
                    }
                    if at == 6000 {
                        break;
                    }
                    let n = stream.input_capacity().min(put).min(6000 - at);
                    assert!(n > 0);
                    let input = (at..at + n).map(music).collect::<Vec<_>>();
                    stream.append(&input).unwrap();
                    at += n;
                    if at == 6000 {
                        stream.finish();
                    }
                }
                assert_eq!(frames, record["frames"].as_u64().unwrap() as usize);
                assert_eq!(
                    hex::encode(hash.finalize()),
                    record["sha256"].as_str().unwrap(),
                    "volume {} chunks {put}/{get}",
                    record["volume"]
                );
                assert_eq!(stream.effects().active(), 0);
                assert!(stream.append(&[[0; 2]]).is_err());
                drop(stream);
                assert_eq!(budget.used(), 0);
            }
        }
    }
    #[test]
    fn denied_buffers_do_not_advance_effects_or_leak_reservations() {
        assert!(Stream::new(&memory_budget::Budget::new(1)).is_err());
        let budget = memory_budget::Budget::new(128 * 1024);
        let mut stream = Stream::new(&budget).unwrap();
        let clip = audio_voices::Clip::new(&[123, -456], &budget).unwrap();
        stream.effects().play(clip);
        assert!(stream.append(&[[0; 2]; 801]).is_err());
        assert_eq!(stream.effects().active(), 1);
        assert!(stream.render(&mut [[0; 2]; 801]).is_err());
        stream.append(&[[0; 2]]).unwrap();
        assert_eq!(stream.effects().active(), 0);
        drop(stream);
        assert_eq!(budget.used(), 0);
    }
    #[cfg(feature = "audio")]
    #[test]
    fn device_clock_adapter_keeps_effects_running_after_music_ends() {
        let budget = memory_budget::Budget::new(128 * 1024);
        assert!(Playback::new(crate::music::fixture_output(&[[0; 2]]), &budget).is_err());
        assert_eq!(budget.used(), 0);
        let source = crate::music::fixture_output_at_rate(&[[0; 2]], 44100);
        let mut playback = Playback::new(source, &budget).unwrap();
        let clip = audio_voices::Clip::new(&[20000; 4000], &budget).unwrap();
        playback.effects().play(clip);
        let mut audible = 0;
        for _ in 0..5000 {
            audible += usize::from(playback.frame().unwrap() != [0; 2]);
        }
        assert!(audible > 2000);
        assert_eq!(playback.effects().active(), 0);
        drop(playback);
        assert_eq!(budget.used(), 0);
    }
}
