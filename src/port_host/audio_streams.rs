//! Session-owned PCM streams. Device consumption is distinct from physical
//! playback; cancellation cannot recall samples already handed to a driver.
use super::{identifier, Error, Result};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    Mono,
    Stereo,
    Surround51,
}
impl Layout {
    pub fn channels(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::Surround51 => 6,
        }
    }
    fn decode(value: u32) -> Result<Self> {
        match value {
            0 => Ok(Self::Mono),
            1 => Ok(Self::Stereo),
            2 => Ok(Self::Surround51),
            _ => Err(Error::Unsupported),
        }
    }
}
struct Stream {
    layout: Layout,
    queue: VecDeque<[i16; 6]>,
    capacity: usize,
    charge: u64,
    running: bool,
    submitted: u64,
    consumed: u64,
    cancelled: u64,
}
struct State {
    closed: bool,
    used: u64,
    streams: BTreeMap<u64, Stream>,
}
#[derive(Clone)]
pub struct Mixer {
    state: Arc<Mutex<State>>,
    layout: Layout,
    budget: u64,
}
impl Mixer {
    /// Output layout is a HOST declaration of actual speaker order, never
    /// inferred from a game's request or merely from device channel count.
    /// 5.1 order: FL,FR,FC,LFE,BL,BR. No implicit 5.1-to-stereo downmix.
    pub fn new(layout: Layout, budget: u64) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                closed: false,
                used: 0,
                streams: BTreeMap::new(),
            })),
            layout,
            budget,
        }
    }
    pub fn layout(&self) -> Layout {
        self.layout
    }
    pub fn call(&self, operation: u32, bytes: &[u8], capacity: usize) -> Result<Vec<u8>> {
        let mut r = super::protocol::Request(bytes);
        let mut state = self.state.lock().map_err(|_| Error::Failed)?;
        if state.closed {
            return Err(Error::Denied);
        }
        match operation {
            130 => {
                if capacity < 8 {
                    return Err(Error::Invalid);
                }
                let format = r.u32()?;
                let rate = r.u32()?;
                let layout = Layout::decode(r.u32()?)?;
                let frames = r.u32()? as usize;
                r.end()?;
                if format != 0 || rate != 48000 {
                    return Err(Error::Unsupported);
                }
                if frames == 0 || frames > 48000 {
                    return Err(Error::Invalid);
                }
                if layout == Layout::Surround51 && self.layout != Layout::Surround51 {
                    return Err(Error::Unsupported);
                }
                if self.layout == Layout::Mono && layout != Layout::Mono {
                    return Err(Error::Unsupported);
                }
                let charge = frames as u64 * 12 + 256;
                if state.streams.len() >= 16 || charge > self.budget.saturating_sub(state.used) {
                    return Err(Error::Busy);
                }
                let id = identifier()?;
                state.streams.insert(
                    id,
                    Stream {
                        layout,
                        queue: VecDeque::with_capacity(frames),
                        capacity: frames,
                        charge,
                        running: false,
                        submitted: 0,
                        consumed: 0,
                        cancelled: 0,
                    },
                );
                state.used += charge;
                Ok(id.to_le_bytes().to_vec())
            }
            131 => {
                let id = r.u64()?;
                let stream = state.streams.get_mut(&id).ok_or(Error::Denied)?;
                let channels = stream.layout.channels();
                if r.0.is_empty()
                    || r.0.len() % (2 * channels) != 0
                    || r.0.len() > 1600 * channels * 2
                {
                    return Err(Error::Invalid);
                }
                let frames = r.0.len() / (channels * 2);
                if frames > stream.capacity - stream.queue.len() {
                    return Err(Error::Busy);
                }
                let submitted = stream
                    .submitted
                    .checked_add(frames as u64)
                    .ok_or(Error::Busy)?;
                for frame in r.0.chunks_exact(channels * 2) {
                    let mut sample = [0; 6];
                    for (index, value) in frame.chunks_exact(2).enumerate() {
                        sample[index] = i16::from_le_bytes(value.try_into().unwrap());
                    }
                    if stream.layout == Layout::Mono && self.layout != Layout::Mono {
                        sample[1] = sample[0];
                    }
                    stream.queue.push_back(sample);
                }
                stream.submitted = submitted;
                Ok(Vec::new())
            }
            132 => {
                if capacity < 40 {
                    return Err(Error::Invalid);
                }
                let id = r.u64()?;
                r.end()?;
                let s = state.streams.get(&id).ok_or(Error::Denied)?;
                let mut result = Vec::new();
                for value in [s.submitted, s.consumed, s.cancelled, s.queue.len() as u64] {
                    result.extend(value.to_le_bytes());
                }
                result.extend(u32::from(s.running).to_le_bytes());
                result.extend((s.layout.channels() as u32).to_le_bytes());
                Ok(result)
            }
            133 => {
                let id = r.u64()?;
                let action = r.u32()?;
                r.end()?;
                if action > 3 {
                    return Err(Error::Invalid);
                }
                let s = state.streams.get_mut(&id).ok_or(Error::Denied)?;
                if action >= 2 {
                    s.cancelled += s.queue.len() as u64;
                    s.queue.clear();
                }
                match action {
                    0 | 3 => s.running = false,
                    1 => s.running = true,
                    _ => (),
                };
                Ok(Vec::new())
            }
            134 => {
                let id = r.u64()?;
                r.end()?;
                let s = state.streams.remove(&id).ok_or(Error::Denied)?;
                state.used -= s.charge;
                Ok(Vec::new())
            }
            _ => Err(Error::Unsupported),
        }
    }
    /// Called only by the host audio callback at the 48kHz source rate.
    /// Saturate after summing all independent streams; a paused stream keeps
    /// its queue and counters. Underflow consumes no nonexistent frame.
    pub fn frame(&self) -> [i16; 6] {
        let Ok(mut state) = self.state.lock() else {
            return [0; 6];
        };
        if state.closed {
            return [0; 6];
        }
        let mut mixed = [0i64; 6];
        for s in state.streams.values_mut() {
            if s.running {
                if let Some(frame) = s.queue.pop_front() {
                    for (to, from) in mixed.iter_mut().zip(frame) {
                        *to += i64::from(from);
                    }
                    s.consumed += 1;
                }
            }
        }
        mixed.map(|v| v.clamp(i16::MIN.into(), i16::MAX.into()) as i16)
    }
    pub fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
            state.streams.clear();
            state.used = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn create(m: &Mixer, layout: u32, frames: u32) -> Result<u64> {
        let request = [0u32, 48000, layout, frames]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        Ok(u64::from_le_bytes(
            m.call(130, &request, 8)?.try_into().unwrap(),
        ))
    }
    fn action(m: &Mixer, id: u64, action: u32) {
        m.call(
            133,
            &[id.to_le_bytes().as_slice(), &action.to_le_bytes()].concat(),
            0,
        )
        .unwrap();
    }
    fn push(m: &Mixer, id: u64, samples: &[i16]) -> Result<Vec<u8>> {
        let mut bytes = id.to_le_bytes().to_vec();
        bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
        m.call(131, &bytes, 0)
    }
    #[test]
    fn streams_start_paused_mix_independently_cancel_and_account_exactly() {
        let m = Mixer::new(Layout::Stereo, 65536);
        let a = create(&m, 1, 2).unwrap();
        let b = create(&m, 0, 2).unwrap();
        push(&m, a, &[100, 200, 300, 400]).unwrap();
        push(&m, b, &[10, 20]).unwrap();
        assert_eq!(m.frame(), [0; 6]);
        assert_eq!(push(&m, a, &[1, 2]), Err(Error::Busy));
        action(&m, a, 1);
        action(&m, b, 1);
        assert_eq!(m.frame(), [110, 210, 0, 0, 0, 0]);
        action(&m, a, 3);
        assert_eq!(m.frame(), [20, 20, 0, 0, 0, 0]);
        let status = m.call(132, &a.to_le_bytes(), 40).unwrap();
        let counts = status[..32]
            .chunks_exact(8)
            .map(|v| u64::from_le_bytes(v.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(counts, [2, 1, 1, 0]);
        m.call(134, &a.to_le_bytes(), 0).unwrap();
        assert_eq!(push(&m, a, &[1, 2]), Err(Error::Denied));
        m.close();
        assert_eq!(m.frame(), [0; 6]);
        assert_eq!(create(&m, 1, 1), Err(Error::Denied));
    }
    #[test]
    fn layouts_channels_saturation_limits_handles_and_malformed_packets() {
        let m = Mixer::new(Layout::Surround51, 65536);
        let id = create(&m, 2, 2).unwrap();
        push(&m, id, &[1, 2, 3, 4, 5, 6]).unwrap();
        action(&m, id, 1);
        assert_eq!(m.frame(), [1, 2, 3, 4, 5, 6]);
        assert_eq!(push(&m, id, &[1, 2]), Err(Error::Invalid));
        assert_eq!(m.call(132, &id.to_le_bytes(), 39), Err(Error::Invalid));
        let stereo = Mixer::new(Layout::Stereo, 65536);
        assert_eq!(create(&stereo, 2, 1), Err(Error::Unsupported));
        assert_eq!(create(&stereo, 9, 1), Err(Error::Unsupported));
        assert_eq!(create(&stereo, 1, 0), Err(Error::Invalid));
        assert_eq!(push(&stereo, id, &[1, 2]), Err(Error::Denied));
        assert_eq!(
            create(&Mixer::new(Layout::Stereo, 1), 1, 1),
            Err(Error::Busy)
        );
        for _ in 0..16 {
            create(&stereo, 1, 1).unwrap();
        }
        assert_eq!(create(&stereo, 1, 1), Err(Error::Busy));
        let saturation = Mixer::new(Layout::Stereo, 65536);
        for _ in 0..2 {
            let id = create(&saturation, 1, 1).unwrap();
            push(&saturation, id, &[30000, -30000]).unwrap();
            action(&saturation, id, 1);
        }
        assert_eq!(saturation.frame(), [32767, -32768, 0, 0, 0, 0]);
    }
}
