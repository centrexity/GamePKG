//! Keep engine/network/audio progress independent of desktop painting.
use super::{protocol, session::Session};
use anyhow::{ensure, Result};
use std::{
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

const INPUT_LEASE: Duration = Duration::from_millis(250);
#[derive(Default)]
struct Input {
    held: u32,
    pressed: u32,
    dx: i32,
    dy: i32,
    events: Vec<Vec<u8>>,
    updated: Option<Instant>,
    expired: bool,
    reset_pending: bool,
    resize: Option<Vec<u8>>,
    modifiers: Option<Vec<u8>>,
}
impl Input {
    fn submit(&mut self, buttons: u32, dx: i32, dy: i32, events: Vec<Vec<u8>>) -> Result<()> {
        ensure!(
            self.events.len()
                + events
                    .iter()
                    .filter(|e| !e.starts_with(b"GPD1") && !e.starts_with(b"GPM1"))
                    .count()
                <= protocol::MAX_QUEUED_EVENTS,
            "Cube 2 input event queue exceeded its bound"
        );
        self.pressed |= buttons & !self.held;
        self.held = buttons;
        self.dx = self.dx.saturating_add(dx).clamp(-2048, 2048);
        self.dy = self.dy.saturating_add(dy).clamp(-2048, 2048);
        for event in events {
            if event.starts_with(b"GPD1") {
                self.resize = Some(event);
            } else if event.starts_with(b"GPM1") {
                self.modifiers = Some(event);
            } else {
                self.events.push(event);
            }
        }
        self.updated = Some(Instant::now());
        self.expired = false;
        Ok(())
    }
    fn reset(&mut self) {
        self.held = 0;
        self.pressed = 0;
        self.dx = 0;
        self.dy = 0;
        self.events.clear();
        self.modifiers = None;
        self.expired = true;
        self.reset_pending = true;
    }
    fn take(&mut self, now: Instant) -> (u32, i32, i32, Vec<Vec<u8>>) {
        if !self.expired
            && self
                .updated
                .is_some_and(|at| now.saturating_duration_since(at) >= INPUT_LEASE)
        {
            // Release stale held controls, while preserving queued one-shot
            // inputs that arrived during a slow engine tick.
            self.held = 0;
            self.modifiers = None;
            self.expired = true;
            self.reset_pending = true;
        }
        let buttons = self.held | std::mem::take(&mut self.pressed);
        let reserved = usize::from(self.reset_pending)
            + usize::from(self.resize.is_some())
            + usize::from(self.modifiers.is_some());
        let count = self.events.len().min(protocol::MAX_EVENTS - reserved);
        let mut events: Vec<_> = self.events.drain(..count).collect();
        if self.reset_pending && events.len() < protocol::MAX_EVENTS {
            events.push(protocol::RESET_INPUT.to_vec());
            self.reset_pending = false;
        }
        if events.len() < protocol::MAX_EVENTS {
            if let Some(resize) = self.resize.take() {
                events.push(resize);
            }
        }
        if let Some(modifiers) = self.modifiers.take() {
            events.push(modifiers);
        }
        if self.expired && !self.events.is_empty() {
            self.reset_pending = true;
        }
        (
            buttons,
            std::mem::take(&mut self.dx),
            std::mem::take(&mut self.dy),
            events,
        )
    }
}
#[derive(Default)]
pub(super) struct Output {
    pub frame: Option<crate::drawing::Frame>,
    pub audio: Vec<i16>,
    pub audio_channels: u32,
    pub exited: bool,
    pub error: Option<String>,
    pub ticks: u64,
    pub frame_size: (u32, u32),
}
impl Output {
    fn publish(&mut self, frame: crate::drawing::Frame, audio: Vec<i16>, channels: u32) {
        self.frame = Some(frame);
        self.ticks = self.ticks.saturating_add(1);
        if self.audio_channels != channels {
            self.audio.clear();
            self.audio_channels = channels;
        }
        self.audio.extend(audio);
        let maximum = protocol::MAX_AUDIO_SAMPLES as usize * channels as usize;
        if self.audio.len() > maximum {
            self.audio.drain(..self.audio.len() - maximum);
        }
    }
}
#[derive(Default)]
struct Shared {
    input: Input,
    output: Output,
    stop: bool,
    #[cfg(feature = "audio")]
    audio: Option<crate::audio::Sink>,
}
pub(super) struct Worker {
    shared: Arc<Mutex<Shared>>,
    thread: Option<JoinHandle<Result<()>>>,
}
impl Worker {
    pub fn start(
        mut session: Session,
        mut save: super::save::Store,
        mut progress: Option<crate::progress::Session>,
    ) -> Result<Self> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let state = shared.clone();
        let thread = std::thread::Builder::new()
            .name("cube2-player".into())
            .spawn(move || {
                if let Some(progress) = &mut progress {
                    progress.start();
                }
                let run = (|| -> Result<()> {
                    loop {
                        let started = Instant::now();
                        let (buttons, dx, dy, events) = {
                            let mut state = state
                                .lock()
                                .map_err(|_| anyhow::anyhow!("Cube 2 input lock failed"))?;
                            if state.stop {
                                break;
                            }
                            state.input.take(started)
                        };
                        for event in events {
                            session.queue(event)?;
                        }
                        let frame = session.tick_update(buttons, dx, dy)?;
                        let Some(frame) = frame else {
                            state
                                .lock()
                                .map_err(|_| anyhow::anyhow!("Cube 2 output lock failed"))?
                                .output
                                .exited = true;
                            break;
                        };
                        let frame_size = (frame.width, frame.height);
                        let drawing = frame.drawing_frame()?;
                        let channels = frame.audio_channels;
                        let mut audio = frame.audio;
                        #[cfg(feature = "audio")]
                        if let Some(sink) = state
                            .lock()
                            .map_err(|_| anyhow::anyhow!("Cube 2 audio lock failed"))?
                            .audio
                            .clone()
                        {
                            for samples in audio.chunks(1600 * channels as usize) {
                                sink.submit_channels(samples, channels);
                            }
                            audio.clear();
                        }
                        {
                            let mut shared = state
                                .lock()
                                .map_err(|_| anyhow::anyhow!("Cube 2 output lock failed"))?;
                            shared.output.frame_size = frame_size;
                            shared
                                .output
                                .publish(drawing, std::mem::take(&mut audio), channels);
                        }
                        if let Some(progress) = &mut progress {
                            progress.tick(&[])?;
                        }
                        // The original engine's limitfps/menufps owns pacing.
                        // Do not impose an additional fixed 60 Hz game clock.
                    }
                    Ok(())
                })();
                if let Err(error) = &run {
                    if let Ok(mut s) = state.lock() {
                        s.output.error = Some(format!("{error:#}"));
                    }
                }
                // Stop the play clock before save/close work or any later UI delay.
                let history = progress
                    .as_mut()
                    .map_or(Ok(()), crate::progress::Session::finish);
                let snapshot = session.snapshot();
                let close = session.close();
                let saved = snapshot.and_then(|bytes| save.commit(&bytes));
                let result = run.and(history).and(saved).and(close);
                if let Err(error) = &result {
                    if let Ok(mut s) = state.lock() {
                        s.output.error = Some(format!("{error:#}"));
                    }
                }
                result
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }
    pub fn submit(&self, buttons: u32, dx: i32, dy: i32, events: Vec<Vec<u8>>) -> Result<()> {
        self.shared
            .lock()
            .map_err(|_| anyhow::anyhow!("Cube 2 input lock failed"))?
            .input
            .submit(buttons, dx, dy, events)
    }
    pub fn reset(&self) {
        if let Ok(mut s) = self.shared.lock() {
            s.input.reset();
        }
    }
    pub fn poll(&self) -> Result<Output> {
        let mut state = self
            .shared
            .lock()
            .map_err(|_| anyhow::anyhow!("Cube 2 output lock failed"))?;
        Ok(Output {
            frame: state.output.frame.take(),
            audio: std::mem::take(&mut state.output.audio),
            audio_channels: state.output.audio_channels,
            exited: state.output.exited,
            error: state.output.error.take(),
            ticks: state.output.ticks,
            frame_size: state.output.frame_size,
        })
    }
    #[cfg(all(feature = "audio", feature = "gui"))]
    pub fn audio(&self, sink: Option<crate::audio::Sink>) {
        if let Ok(mut state) = self.shared.lock() {
            state.audio = sink;
            state.output.audio.clear();
        }
    }
    pub fn finish(&mut self) -> Result<()> {
        if let Ok(mut state) = self.shared.lock() {
            state.stop = true;
        }
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| anyhow::anyhow!("Cube 2 player worker failed"))??;
        }
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn resize_bursts_keep_only_the_latest_size_and_survive_input_release() {
        let mut input = super::Input::default();
        for width in 640..800 {
            input
                .submit(
                    0,
                    0,
                    0,
                    vec![super::protocol::resize_event(width, 480).unwrap()],
                )
                .unwrap();
        }
        input.reset();
        let (_, _, _, events) = input.take(std::time::Instant::now());
        assert_eq!(events.iter().filter(|e| e.starts_with(b"GPD1")).count(), 1);
        assert!(events.contains(&super::protocol::resize_event(799, 480).unwrap()));
        assert!(input.take(std::time::Instant::now()).3.is_empty());
    }
    use super::*;
    #[test]
    fn idle_updates_during_loading_are_coalesced_and_keys_stay_ordered() {
        let mut input = Input::default();
        let mut expected = Vec::new();
        for i in 0..200 {
            let key = protocol::key_event(40, i % 2 == 0, false, 0).unwrap();
            expected.push(key.clone());
            input
                .submit(0, 0, 0, vec![protocol::modifiers_event(0).unwrap(), key])
                .unwrap();
        }
        assert_eq!(input.events.len(), 200);
        let mut delivered = Vec::new();
        while !input.events.is_empty() {
            let events = input.take(Instant::now()).3;
            assert!(events.len() <= protocol::MAX_EVENTS);
            delivered.extend(events.into_iter().filter(|e| e.starts_with(b"GPI1")));
        }
        assert_eq!(delivered, expected);
    }
    #[test]
    fn rapid_press_release_is_latched_and_stale_inputs_expire_once() {
        let mut input = Input::default();
        input.submit(1, 100, 0, Vec::new()).unwrap();
        input.submit(0, 100, 0, Vec::new()).unwrap();
        assert_eq!(input.take(Instant::now()), (1, 200, 0, Vec::new()));
        assert_eq!(input.take(Instant::now()).0, 0);
        input
            .submit(
                1,
                3,
                4,
                vec![protocol::key_event(23, true, false, 0).unwrap()],
            )
            .unwrap();
        let time = input.updated.unwrap() + INPUT_LEASE;
        let (buttons, x, y, events) = input.take(time);
        assert_eq!((buttons, x, y), (1, 3, 4));
        assert_eq!(
            events,
            vec![
                protocol::key_event(23, true, false, 0).unwrap(),
                protocol::RESET_INPUT.to_vec()
            ]
        );
        assert_eq!(input.take(time + INPUT_LEASE).0, 0);
        assert!(input.take(time + INPUT_LEASE).3.is_empty());
    }
    #[test]
    fn queues_are_bounded_and_old_frames_audio_are_replaced() {
        let mut input = Input::default();
        input
            .submit(
                0,
                0,
                0,
                vec![protocol::RESET_INPUT.to_vec(); protocol::MAX_QUEUED_EVENTS],
            )
            .unwrap();
        assert!(input
            .submit(1, 0, 0, vec![protocol::RESET_INPUT.to_vec()])
            .is_err());
        assert_eq!(input.events.len(), protocol::MAX_QUEUED_EVENTS);
        assert_eq!(input.held, 0);
        let time = input.updated.unwrap() + INPUT_LEASE;
        assert_eq!(input.take(time).3.len(), protocol::MAX_EVENTS);
        while !input.events.is_empty() {
            let events = input.take(time).3;
            assert!(!events.is_empty() && events.len() <= protocol::MAX_EVENTS);
        }
        assert!(input.take(time).3.is_empty());
        let mut output = Output::default();
        for i in 0..100 {
            let mut frame = crate::drawing::Frame::default();
            frame.background = Some(i);
            output.publish(frame, vec![i as i16; 4800], 1);
        }
        assert_eq!(output.frame.unwrap().background, Some(99));
        assert_eq!(output.audio, vec![99; 4800]);
        assert_eq!(output.ticks, 100);
    }
}
