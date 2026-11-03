/*
  Simple DirectMedia Layer
  Copyright (C) 1997-2024 Sam Lantinga <slouken@libsdl.org>

  This software is provided 'as-is', without any express or implied
  warranty.  In no event will the authors be held liable for any damages
  arising from the use of this software.

  Permission is granted to anyone to use this software for any purpose,
  including commercial applications, and to alter it and redistribute it
  freely, subject to the following restrictions:

  1. The origin of this software must not be misrepresented; you must not
     claim that you wrote the original software. If you use this software
     in a product, an acknowledgment in the product documentation would be
     appreciated but is not required.
  2. Altered source versions must be plainly marked as such, and must not be
     misrepresented as being the original software.
  3. This notice may not be removed or altered from any source distribution.
*/

// Altered version: streaming Rust adaptation of SDL 2.30.0 audio conversion.
//! Fixed-memory streaming form of SDL 2.30's bandlimited stereo conversion.
//! Shared by host music, combined playback and the trusted browser converter.
#[path = "audio_resample_filter.rs"]
mod coefficients;
use coefficients::{DIFFERENCE, FILTER};
const CAPACITY: usize = 2048;
const MAX_FRAMES: usize = 800;
const RATES: [u32; 9] = [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000];

pub struct Resampler {
    input_rate: u32,
    output_rate: u32,
    frames: [[f32; 2]; CAPACITY],
    first: u64,
    received: u64,
    produced: u64,
    finished: bool,
}

/// SDL_mixer 2.8's 4,096-byte S16 source puts into SDL 2.30 AudioStream
/// (1,024 stereo frames or 2,048 mono frames).
/// Phase restarts at the original internal boundaries, including its final
/// staging/flush behavior. Consumer read sizes never move those boundaries.
pub struct MixerResampler {
    inner: Resampler,
    put: u64,
    processed: u64,
    first_put: bool,
    stage: MixerStage,
    block_start: u64,
    block_out: u64,
    block_at: u64,
    put_frames: u64,
    announced: Option<u64>,
    announced_final: bool,
}
enum MixerStage {
    Puts,
    FlushFirst(u64),
    FlushSecond(u64),
    Done,
}
impl MixerResampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, &'static str> {
        Ok(Self {
            inner: Resampler::new(input_rate, output_rate)?,
            put: 0,
            processed: 0,
            first_put: true,
            stage: MixerStage::Puts,
            block_start: 0,
            block_out: 0,
            block_at: 0,
            put_frames: 1024,
            announced: None,
            announced_final: false,
        })
    }
    /// Original 4,096-byte S16 mono puts contain 2,048 source frames.
    /// A trusted producer stages each complete put before announcing it, then
    /// feeds it with backpressure. The existing ring need not hold the whole
    /// put plus retained padding at once.
    pub fn new_mono(input_rate: u32, output_rate: u32) -> Result<Self, &'static str> {
        let mut result = Self::new(input_rate, output_rate)?;
        result.put_frames = 2048;
        result.announced = Some(0);
        Ok(result)
    }
    pub fn announce_put(&mut self, frames: usize, final_put: bool) -> Result<(), &'static str> {
        let previous = self.announced.ok_or("explicit puts require mono mode")?;
        if self.inner.finished
            || self.announced_final
            || previous != self.inner.received
            || frames == 0
            || frames > 2048
            || (frames < 2048 && !final_put)
        {
            return Err("invalid mono source put");
        }
        let end = previous
            .checked_add(frames as u64)
            .ok_or("music put overflow")?;
        if u128::from(end) * u128::from(self.inner.output_rate) / u128::from(self.inner.input_rate)
            > u128::from(u64::MAX)
        {
            return Err("music put timeline overflow");
        }
        self.announced = Some(end);
        self.announced_final = final_put;
        Ok(())
    }
    pub fn input_capacity(&self) -> usize {
        if self.inner.finished {
            return 0;
        }
        (CAPACITY - (self.inner.received - self.inner.first) as usize).min(MAX_FRAMES)
    }
    pub fn append(&mut self, input: &[[f32; 2]]) -> Result<(), &'static str> {
        self.inner.append(input)
    }
    pub fn finish(&mut self) {
        self.inner.finish();
    }
    fn padding(&self) -> u64 {
        if self.inner.input_rate > self.inner.output_rate {
            (512 * u64::from(self.inner.input_rate)).div_ceil(u64::from(self.inner.output_rate))
        } else {
            512
        }
    }
    fn block(&mut self, frames: u64, limit: u64) -> Result<(), &'static str> {
        self.block_start = self.processed;
        self.processed = self
            .processed
            .checked_add(frames)
            .ok_or("music block overflow")?;
        self.block_out = ((u128::from(frames) * u128::from(self.inner.output_rate)
            / u128::from(self.inner.input_rate)) as u64)
            .min(limit);
        self.block_at = 0;
        Ok(())
    }
    fn prepare(&mut self) -> Result<bool, &'static str> {
        while self.block_at == self.block_out {
            let padding = self.padding();
            match self.stage {
                MixerStage::Done => return Ok(false),
                MixerStage::Puts => {
                    let tail = self
                        .announced
                        .unwrap_or(self.inner.received)
                        .saturating_sub(self.put);
                    let amount = if tail >= self.put_frames {
                        self.put_frames
                    } else {
                        if !self.inner.finished && !self.announced_final {
                            return Ok(false);
                        }
                        if tail == 0 {
                            self.stage = MixerStage::Done;
                            continue;
                        }
                        if tail < padding {
                            let actual = tail + if self.first_put { 0 } else { padding };
                            let remaining = (actual * u64::from(self.inner.output_rate))
                                .div_ceil(u64::from(self.inner.input_rate));
                            self.stage = MixerStage::FlushFirst(remaining);
                            continue;
                        }
                        self.stage = MixerStage::Done;
                        tail
                    };
                    self.put += amount;
                    let frames = amount - if self.first_put { padding } else { 0 };
                    self.first_put = false;
                    self.block(frames, u64::MAX)?;
                }
                MixerStage::FlushFirst(remaining) => {
                    self.block(if self.first_put { 0 } else { padding }, remaining)?;
                    self.first_put = false;
                    self.stage = MixerStage::FlushSecond(remaining - self.block_out);
                }
                MixerStage::FlushSecond(remaining) => {
                    self.block(padding, remaining)?;
                    self.stage = MixerStage::Done;
                }
            }
            // If a zero-length block was consumed, release its obsolete history.
            if self.block_out == 0 {
                self.inner.first = self.processed.saturating_sub(5).min(self.inner.received);
            }
        }
        Ok(true)
    }
    pub fn render(&mut self, output: &mut [[f32; 2]]) -> Result<usize, &'static str> {
        if self.inner.input_rate == self.inner.output_rate {
            return self.inner.render(output);
        }
        if output.is_empty() || output.len() > MAX_FRAMES {
            return Err("invalid mixer output buffer");
        }
        let mut count = 0;
        for frame in output {
            if !self.prepare()? {
                break;
            }
            let numerator = self.block_at * u64::from(self.inner.input_rate);
            let relative = numerator / u64::from(self.inner.output_rate);
            let fraction = (numerator % u64::from(self.inner.output_rate)) as u32;
            if self.announced.is_some()
                && !self.inner.finished
                && (self.block_start + relative).saturating_add(6) >= self.inner.received
            {
                break;
            }
            *frame = self.inner.filtered(self.block_start + relative, fraction);
            self.block_at += 1;
            count += 1;
            let next = if self.block_at == self.block_out {
                self.processed
            } else {
                self.block_start
                    + self.block_at * u64::from(self.inner.input_rate)
                        / u64::from(self.inner.output_rate)
            };
            self.inner.first = next.saturating_sub(5).min(self.inner.received);
        }
        Ok(count)
    }
}
impl Resampler {
    pub fn new(input_rate: u32, output_rate: u32) -> Result<Self, &'static str> {
        if !RATES.contains(&input_rate) || ![44100, 48000].contains(&output_rate) {
            return Err("unsupported music conversion rate");
        }
        Ok(Self {
            input_rate,
            output_rate,
            frames: [[0.0; 2]; CAPACITY],
            first: 0,
            received: 0,
            produced: 0,
            finished: false,
        })
    }
    /// Rejection never changes queued input or the timeline.
    pub fn append(&mut self, input: &[[f32; 2]]) -> Result<(), &'static str> {
        if self.finished
            || input.is_empty()
            || input.len() > MAX_FRAMES
            || self.received - self.first + input.len() as u64 > CAPACITY as u64
            || input.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("invalid or full music conversion input");
        }
        let end = self
            .received
            .checked_add(input.len() as u64)
            .ok_or("music timeline overflow")?;
        if u128::from(end) * u128::from(self.output_rate) / u128::from(self.input_rate)
            > u128::from(u64::MAX)
        {
            return Err("music output timeline overflow");
        }
        for (index, frame) in input.iter().enumerate() {
            self.frames[((self.received + index as u64) % CAPACITY as u64) as usize] = *frame;
        }
        self.received = end;
        Ok(())
    }
    pub fn input_capacity(&self) -> usize {
        if self.finished {
            return 0;
        }
        (CAPACITY - (self.received - self.first) as usize).min(MAX_FRAMES)
    }
    pub fn finish(&mut self) {
        self.finished = true;
    }
    fn source_index(&self) -> u64 {
        ((u128::from(self.produced) * u128::from(self.input_rate)) / u128::from(self.output_rate))
            as u64
    }
    fn sample_frame(&self, index: Option<u64>) -> [f32; 2] {
        match index {
            Some(index) if index >= self.first && index < self.received => {
                self.frames[(index % CAPACITY as u64) as usize]
            }
            _ => [0.0; 2],
        }
    }
    #[inline]
    fn filtered(&self, source: u64, fraction: u32) -> [f32; 2] {
        let left = fraction as f32 / self.output_rate as f32;
        let right = 1.0 - left;
        let left_index = (fraction * 512 / self.output_rate) as usize;
        let right_index = ((self.output_rate - fraction) * 512 / self.output_rate) as usize;
        let mut frame = [0.0; 2];
        // Both channels share phase, coefficients and ring positions. Keep the
        // original left-wing/right-wing addition order within each channel;
        // do not reassociate sums or fuse multiply/adds to gain speed.
        for (index, fraction, right_wing) in [(left_index, left, false), (right_index, right, true)]
        {
            let mut j = 0usize;
            while index + j * 512 < FILTER.len() {
                let filter = index + j * 512;
                let at = if right_wing {
                    source.checked_add(1 + j as u64)
                } else {
                    source.checked_sub(j as u64)
                };
                let samples = self.sample_frame(at);
                let weight = FILTER[filter] + fraction * DIFFERENCE[filter];
                frame[0] += samples[0] * weight;
                frame[1] += samples[1] * weight;
                j += 1;
            }
        }
        frame
    }
    /// Zero means more input is needed, or the finished stream is drained.
    /// Phase is global integer time, independent of append/render chunk sizes.
    #[inline]
    pub fn render(&mut self, output: &mut [[f32; 2]]) -> Result<usize, &'static str> {
        if output.is_empty() || output.len() > MAX_FRAMES {
            return Err("invalid music conversion output");
        }
        if self.input_rate == self.output_rate {
            // Equal-rate output is the original PCM, including its exact bit
            // patterns. Copy at most two contiguous ring spans instead of
            // recomputing integer phase and bounds for every stereo frame.
            let count = (self.received - self.produced).min(output.len() as u64) as usize;
            if count == 0 {
                return Ok(0);
            }
            let start = (self.produced % CAPACITY as u64) as usize;
            let tail = count.min(CAPACITY - start);
            output[..tail].copy_from_slice(&self.frames[start..start + tail]);
            output[tail..count].copy_from_slice(&self.frames[..count - tail]);
            self.produced += count as u64;
            // Retain the original queue/backpressure contract, even though
            // equal-rate output itself does not need this filter history.
            self.first = self.produced.saturating_sub(5).min(self.received);
            return Ok(count);
        }
        let available =
            u128::from(self.received) * u128::from(self.output_rate) / u128::from(self.input_rate);
        let mut count = 0;
        for frame in output.iter_mut() {
            if u128::from(self.produced) >= available {
                break;
            }
            let source = self.source_index();
            // Five filter lobes on each side; defer until the right wing
            // is present. End-of-stream uses SDL's zero edge padding.
            if !self.finished && source.saturating_add(6) >= self.received {
                break;
            }
            let fraction = ((u128::from(self.produced) * u128::from(self.input_rate))
                % u128::from(self.output_rate)) as u32;
            *frame = self.filtered(source, fraction);
            self.produced += 1;
            count += 1;
            self.first = self.source_index().saturating_sub(5).min(self.received);
        }
        Ok(count)
    }
}
