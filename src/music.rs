//! Host-only asynchronous music playback. Game SDK integration is still pending.
use crate::{audio_decode, audio_resample::Resampler, memory_budget};
use anyhow::{ensure, Context, Result};
use std::{
    io::Read,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Arc, Mutex,
    },
};
mod ring;
use ring::Ring;
pub const HOST_BYTES: u64 = 65536;
#[cfg(test)]
const QUEUE_FRAMES: usize = 4800;
const BLOCK_FRAMES: usize = ring::FRAMES;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Status {
    Stopped,
    Loading,
    Playing,
    Paused,
    Ended,
    Failed,
}
impl Status {
    fn from_tag(tag: u64) -> Self {
        match tag & 7 {
            0 => Self::Stopped,
            1 => Self::Loading,
            2 => Self::Playing,
            3 => Self::Paused,
            4 => Self::Ended,
            _ => Self::Failed,
        }
    }
    fn active(self) -> bool {
        matches!(self, Self::Loading | Self::Playing | Self::Paused)
    }
}
fn tag(generation: u64, status: Status) -> u64 {
    (generation << 3) | status as u64
}
struct State {
    generation: u64,
    request: Option<String>,
    error: Option<String>,
}
struct Shared {
    state: Mutex<State>,
    ring: Ring,
    status: AtomicU64,
    volume: AtomicU32,
    eof: AtomicU64,
    quit: AtomicBool,
    consumer_generation: AtomicU64,
    played: AtomicU64,
    // All data buffers and queue storage drop before this reservation.
    _reservation: memory_budget::Lease,
}
impl Shared {
    fn generation(&self) -> u64 {
        self.status.load(Ordering::Acquire) >> 3
    }
    fn transition(&self, generation: u64, change: impl Fn(Status) -> Option<Status>) -> bool {
        let mut current = self.status.load(Ordering::Acquire);
        loop {
            if current >> 3 != generation {
                return false;
            }
            let Some(status) = change(Status::from_tag(current)) else {
                return false;
            };
            match self.status.compare_exchange_weak(
                current,
                tag(generation, status),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(next) => current = next,
            }
        }
    }
    fn force_status(&self, status: Status) {
        let mut current = self.status.load(Ordering::Acquire);
        loop {
            match self.status.compare_exchange_weak(
                current,
                tag(current >> 3, status),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(next) => current = next,
            }
        }
    }
}
#[derive(Clone)]
pub struct Output(Arc<Shared>);
pub struct Service {
    output: Output,
    worker: Option<std::thread::JoinHandle<()>>,
}
fn next_generation(state: &mut State) -> Result<u64> {
    let next = state
        .generation
        .checked_add(1)
        .filter(|value| *value <= u64::MAX >> 3)
        .context("music request counter exhausted")?;
    state.generation = next;
    Ok(next)
}
impl Service {
    pub fn new(
        factory: audio_decode::Factory,
        package: &crate::archive::Package,
        audio_allowed: bool,
        budget: memory_budget::Budget,
        memory_bytes: usize,
    ) -> Result<Self> {
        ensure!(audio_allowed, "music requires audio permission");
        ensure!(
            (65536..=16 * 1048576).contains(&memory_bytes),
            "invalid music decoder budget"
        );
        let reservation = budget.reserve(HOST_BYTES)?;
        let package = package.try_clone_verified()?;
        ensure!(
            std::mem::size_of::<Pipeline>()
                + std::mem::size_of::<Shared>()
                + std::mem::size_of::<[i16; 1600]>()
                + 4096
                <= HOST_BYTES as usize,
            "music host staging exceeds its reservation"
        );
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                generation: 0,
                request: None,
                error: None,
            }),
            ring: Ring::new(),
            status: AtomicU64::new(tag(0, Status::Stopped)),
            volume: AtomicU32::new(128),
            eof: AtomicU64::new(0),
            quit: AtomicBool::new(false),
            consumer_generation: AtomicU64::new(0),
            played: AtomicU64::new(0),
            _reservation: reservation,
        });
        let output = Output(shared.clone());
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("gamepkg-music".into())
            .spawn(move || run(worker_shared, factory, package, budget, memory_bytes))?;
        shared.ring.set_worker(worker.thread().clone());
        Ok(Self {
            output,
            worker: Some(worker),
        })
    }
    pub fn output(&self) -> Output {
        self.output.clone()
    }
    pub fn play(&self, path: &str) -> Result<()> {
        ensure!(crate::archive::valid_path(path), "invalid music asset path");
        let shared = &self.output.0;
        let mut state = shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("music unavailable"))?;
        ensure!(!shared.quit.load(Ordering::Acquire), "music is stopped");
        let generation = next_generation(&mut state)?;
        state.request = Some(path.to_owned());
        state.error = None;
        shared.eof.store(0, Ordering::Release);
        shared
            .status
            .store(tag(generation, Status::Loading), Ordering::Release);
        if shared.quit.load(Ordering::Acquire) {
            shared.force_status(Status::Failed);
            anyhow::bail!("music worker is unavailable");
        }
        shared.ring.wake();
        Ok(())
    }
    pub fn pause(&self, paused: bool) -> Result<()> {
        let shared = &self.output.0;
        let state = shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("music unavailable"))?;
        ensure!(!shared.quit.load(Ordering::Acquire), "music is stopped");
        ensure!(
            shared.transition(state.generation, |status| status.active().then_some(
                if paused {
                    Status::Paused
                } else {
                    Status::Playing
                }
            )),
            "music is not active"
        );
        // The callback never takes this lock. Pause returns after any already
        // committed output read finishes; later reads observe the paused tag.
        if paused {
            shared.ring.wait_consumer();
        }
        shared.ring.wake();
        Ok(())
    }
    pub fn volume(&self, volume: u32) -> Result<()> {
        ensure!(volume <= 128, "music volume must be 0 to 128");
        self.output.0.volume.store(volume, Ordering::Release);
        Ok(())
    }
    pub fn stop(&self) -> Result<()> {
        let shared = &self.output.0;
        let mut state = shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("music unavailable"))?;
        let generation = next_generation(&mut state)?;
        state.request = None;
        state.error = None;
        shared
            .status
            .store(tag(generation, Status::Stopped), Ordering::Release);
        shared.ring.wake();
        Ok(())
    }
    pub fn error(&self) -> Option<String> {
        self.output
            .0
            .state
            .lock()
            .ok()
            .and_then(|state| state.error.clone())
            .or_else(|| {
                (self.output.status() == Status::Failed).then(|| "music worker unavailable".into())
            })
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        let shared = &self.output.0;
        shared.quit.store(true, Ordering::Release);
        shared.force_status(Status::Stopped);
        if let Ok(mut state) = shared.state.lock() {
            state.request = None;
        }
        shared.ring.wake();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Output {
    pub fn status(&self) -> Status {
        Status::from_tag(self.0.status.load(Ordering::Acquire))
    }
    pub fn queued_frames(&self) -> usize {
        self.0.ring.frames()
    }
    pub fn played_frames(&self) -> u64 {
        let current = self.0.status.load(Ordering::Acquire);
        if Status::from_tag(current) == Status::Stopped
            || self.0.consumer_generation.load(Ordering::Acquire) != current >> 3
        {
            return 0;
        }
        self.0.played.load(Ordering::Acquire)
    }
    /// Output-clock consumer. Producer and control locks cannot silence ready
    /// samples. A guard permits one output consumer; it never waits for a lock.
    pub fn read(&self, output: &mut [[i16; 2]]) -> usize {
        if output.is_empty() || output.len() > BLOCK_FRAMES || self.0.quit.load(Ordering::Acquire) {
            return 0;
        }
        let Some(mut consumer) = self.0.ring.consumer() else {
            return 0;
        };
        let current = self.0.status.load(Ordering::Acquire);
        let generation = current >> 3;
        if self.0.consumer_generation.load(Ordering::Relaxed) != generation {
            self.0.played.store(0, Ordering::Relaxed);
            self.0
                .consumer_generation
                .store(generation, Ordering::Release);
        }
        let status = Status::from_tag(current);
        consumer.discard_through(generation, !status.active());
        let mut count = 0;
        for frame in output {
            let current = self.0.status.load(Ordering::Acquire);
            if current >> 3 != generation
                || !matches!(Status::from_tag(current), Status::Loading | Status::Playing)
            {
                break;
            }
            let Some(played) = self.0.played.load(Ordering::Relaxed).checked_add(1) else {
                self.0.transition(generation, |status| {
                    status.active().then_some(Status::Failed)
                });
                break;
            };
            let Some(sample) = consumer.next(generation) else {
                break;
            };
            let volume = self.0.volume.load(Ordering::Acquire) as i32;
            *frame = sample.map(|sample| (i32::from(sample) * volume / 128) as i16);
            self.0.played.store(played, Ordering::Release);
            count += 1;
        }
        if self.0.eof.load(Ordering::Acquire) == generation && self.0.ring.empty() {
            self.0.transition(generation, |status| {
                status.active().then_some(Status::Ended)
            });
        }
        count
    }
}

struct Pipeline {
    stream: audio_decode::Stream,
    converter: Resampler,
    input: [[f32; 2]; BLOCK_FRAMES],
    output: [[f32; 2]; BLOCK_FRAMES],
    decoded: [i16; 1600],
    eof: bool,
}
impl Pipeline {
    fn new(stream: audio_decode::Stream) -> Result<Self> {
        ensure!(
            stream.sample_rate() == 44100 && stream.channels() == 2,
            "unexpected music mixer format"
        );
        Ok(Self {
            stream,
            converter: Resampler::new(44100, 48000).map_err(anyhow::Error::msg)?,
            input: [[0.0; 2]; BLOCK_FRAMES],
            output: [[0.0; 2]; BLOCK_FRAMES],
            decoded: [0; 1600],
            eof: false,
        })
    }
    fn produce(&mut self, output: &mut [i16; 1600]) -> Result<usize> {
        let mut count = 0;
        while count < BLOCK_FRAMES {
            let n = self
                .converter
                .render(&mut self.output[count..])
                .map_err(anyhow::Error::msg)?;
            count += n;
            if count == BLOCK_FRAMES || (n == 0 && self.eof) {
                break;
            }
            if n > 0 {
                continue;
            }
            let capacity = self.converter.input_capacity();
            ensure!(capacity > 0, "music converter cannot progress");
            let n = self.stream.read(&mut self.decoded[..capacity * 2])?;
            if n == 0 {
                self.eof = true;
                self.converter.finish();
                continue;
            }
            for (frame, samples) in self.input.iter_mut().zip(self.decoded[..n].chunks_exact(2)) {
                *frame = [
                    f32::from(samples[0]) / 32768.0,
                    f32::from(samples[1]) / 32768.0,
                ];
            }
            self.converter
                .append(&self.input[..n / 2])
                .map_err(anyhow::Error::msg)?;
        }
        for (frame, samples) in self.output[..count].iter().zip(output.chunks_exact_mut(2)) {
            for channel in 0..2 {
                samples[channel] = (frame[channel] * 32768.0)
                    .round_ties_even()
                    .clamp(-32768.0, 32767.0) as i16;
            }
        }
        Ok(count * 2)
    }
}

enum Action {
    Load(String, u64),
    Fill,
    Reset(u64),
    Quit,
}
fn action(shared: &Shared, generation: u64, active: bool) -> Action {
    loop {
        {
            let Ok(mut state) = shared.state.lock() else {
                return Action::Quit;
            };
            if shared.quit.load(Ordering::Acquire) {
                return Action::Quit;
            }
            if let Some(path) = state.request.take() {
                return Action::Load(path, state.generation);
            }
            let current = shared.status.load(Ordering::Acquire);
            if current >> 3 != generation || (active && !Status::from_tag(current).active()) {
                return Action::Reset(current >> 3);
            }
            if active
                && matches!(Status::from_tag(current), Status::Loading | Status::Playing)
                && (shared.ring.can_write() || shared.ring.exhausted())
            {
                return Action::Fill;
            }
        }
        // Unpark tokens persist if consumption or a control change happens
        // between the predicate check and parking; no notification can be lost.
        std::thread::park();
    }
}
struct Cancellable<'a> {
    input: &'a mut dyn Read,
    shared: &'a Shared,
    generation: u64,
}
impl Read for Cancellable<'_> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if self.shared.quit.load(Ordering::Acquire) || self.shared.generation() != self.generation {
            return Err(std::io::Error::other("music loading cancelled"));
        }
        self.input.read(output)
    }
}
fn failed(shared: &Shared, generation: u64, error: anyhow::Error) {
    if let Ok(mut state) = shared.state.lock() {
        if shared.generation() == generation && !shared.quit.load(Ordering::Acquire) {
            state.error = Some(format!("{error:#}").chars().take(1024).collect());
            shared.transition(generation, |status| {
                status.active().then_some(Status::Failed)
            });
        }
    }
}
struct WorkerExit<'a>(&'a Shared);
impl Drop for WorkerExit<'_> {
    fn drop(&mut self) {
        if !self.0.quit.swap(true, Ordering::AcqRel) {
            self.0.force_status(Status::Failed);
        }
    }
}
fn run(
    shared: Arc<Shared>,
    factory: audio_decode::Factory,
    mut package: crate::archive::Package,
    budget: memory_budget::Budget,
    memory_bytes: usize,
) {
    let _exit = WorkerExit(&shared);
    let mut generation = 0;
    let mut pipeline: Option<Pipeline> = None;
    let mut output = [0i16; 1600];
    loop {
        match action(&shared, generation, pipeline.is_some()) {
            Action::Quit => return,
            Action::Reset(next) => {
                pipeline = None;
                generation = next;
            }
            Action::Load(path, next) => {
                pipeline = None;
                generation = next;
                let loaded = (|| -> Result<Pipeline> {
                    ensure!(
                        package
                            .asset_compression(&path)?
                            .is_none_or(|method| method == zip::CompressionMethod::Stored),
                        "compressed music assets require decompressor memory accounting"
                    );
                    let stream = package.with_asset_reader(&path, 8 * 1048576, |input, size| {
                        let mut input = Cancellable {
                            input,
                            shared: &shared,
                            generation,
                        };
                        factory.open_budgeted(
                            &mut input,
                            size,
                            audio_decode::Output::OriginalMixer(44100),
                            &budget,
                            memory_bytes,
                        )
                    })?;
                    Pipeline::new(stream)
                })();
                match loaded {
                    Ok(loaded) => pipeline = Some(loaded),
                    Err(error) => failed(&shared, generation, error),
                }
            }
            Action::Fill => match pipeline.as_mut().unwrap().produce(&mut output) {
                Ok(0) => {
                    pipeline = None;
                    if shared.generation() == generation && !shared.quit.load(Ordering::Acquire) {
                        shared.eof.store(generation, Ordering::Release);
                        if shared.ring.empty() {
                            shared.transition(generation, |status| {
                                status.active().then_some(Status::Ended)
                            });
                        }
                    }
                }
                Ok(count) => {
                    if shared.generation() == generation && !shared.quit.load(Ordering::Acquire) {
                        if !shared.ring.push(generation, &output[..count]) {
                            pipeline = None;
                            failed(
                                &shared,
                                generation,
                                anyhow::anyhow!("music output queue exhausted"),
                            );
                        } else {
                            shared.transition(generation, |status| {
                                (status == Status::Loading).then_some(Status::Playing)
                            });
                        }
                    }
                }
                Err(error) => {
                    pipeline = None;
                    failed(&shared, generation, error);
                }
            },
        }
    }
}
#[cfg(all(test, feature = "audio"))]
pub(crate) fn fixture_output(frames: &[[i16; 2]]) -> Output {
    let budget = memory_budget::Budget::new(HOST_BYTES);
    let ring = Ring::new();
    for chunk in frames.chunks(BLOCK_FRAMES) {
        let samples = chunk.iter().flatten().copied().collect::<Vec<_>>();
        assert!(ring.push(1, &samples));
    }
    Output(Arc::new(Shared {
        state: Mutex::new(State {
            generation: 1,
            request: None,
            error: None,
        }),
        ring,
        status: AtomicU64::new(tag(1, Status::Playing)),
        volume: AtomicU32::new(128),
        eof: AtomicU64::new(1),
        quit: AtomicBool::new(false),
        consumer_generation: AtomicU64::new(0),
        played: AtomicU64::new(0),
        _reservation: budget.reserve(HOST_BYTES).unwrap(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[cfg(feature = "audio")]
    #[test]
    fn available_music_is_not_replaced_with_silence_while_control_is_locked() {
        let output = fixture_output(&[[123, -456]]);
        let locked = output.0.state.lock().unwrap();
        let reader = output.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let test = std::thread::spawn(move || {
            let mut frame = [[0; 2]; 1];
            let count = reader.read(&mut frame);
            send.send((count, frame)).unwrap();
        });
        let result = receive
            .recv_timeout(std::time::Duration::from_millis(200))
            .unwrap();
        drop(locked);
        test.join().unwrap();
        assert_eq!(result, (1, [[123, -456]]));
    }
    fn factory(frames: u32, infinite: bool) -> audio_decode::Factory {
        let body = if infinite {
            "(loop $forever br $forever) i32.const 0"
        } else {
            "local.get 0 global.get $left i32.lt_u
             if local.get 0 local.set $n else global.get $left local.set $n end
             (block $done (loop $fill
              local.get $i local.get $n i32.ge_u br_if $done
              i32.const 4096 local.get $i i32.const 2 i32.mul i32.add
              i32.const 1 i32.load8_u
              local.get $i i32.const 1 i32.and i32.eqz
              if (result i32) i32.const 1000 else i32.const -2000 end i32.mul i32.store16
              local.get $i i32.const 1 i32.add local.set $i br $fill))
             global.get $left local.get $n i32.sub global.set $left local.get $n"
        };
        audio_decode::Factory::new(
            &wat::parse_str(format!(
                r#"(module
            (memory(export "memory") 1 96)
            (global $left(mut i32)(i32.const 0))
            (func(export "decoder_reserve")(param i32)(result i32)i32.const 1)
            (func(export "decoder_open")(result i32)
              i32.const {samples} global.set $left
              i32.const 1 i32.load8_u i32.eqz if(result i32)i32.const -1 else i32.const 0 end)
            (func(export "decoder_set_mixer_rate")(param i32)(result i32)
              local.get 0 i32.const 44100 i32.eq if(result i32)i32.const 0 else i32.const -1 end)
            (func(export "decoder_read")(param i32)(result i32)(local $n i32)(local $i i32){body})
            (func(export "decoder_output")(result i32)i32.const 4096)
            (func(export "decoder_rate")(result i32)i32.const 44100)
            (func(export "decoder_channels")(result i32)i32.const 2)
            (func(export "decoder_work")(result i32)i32.const 0)
            (func(export "decoder_rewind")(result i32)i32.const 0))"#,
                samples = frames * 2
            ))
            .unwrap(),
        )
        .unwrap()
    }
    fn package(
        method: zip::CompressionMethod,
        verified: bool,
    ) -> (tempfile::TempDir, crate::archive::Package) {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("music.gamepkg");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, byte) in [("a", 1), ("b", 2), ("bad", 0)] {
            writer
                .start_file(
                    format!("assets/{name}.mp3"),
                    zip::write::SimpleFileOptions::default().compression_method(method),
                )
                .unwrap();
            writer.write_all(&[byte]).unwrap();
        }
        writer.finish().unwrap();
        let mut package = crate::archive::Package::open(&path).unwrap();
        if verified {
            package.mark_verified();
        }
        (temporary, package)
    }
    fn wait(mut predicate: impl FnMut() -> bool) {
        let start = std::time::Instant::now();
        while !predicate() {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(2),
                "music worker did not progress"
            );
            std::thread::sleep(std::time::Duration::from_micros(100));
        }
    }
    fn drain(output: &Output) -> Vec<[i16; 2]> {
        let mut result = Vec::new();
        let mut block = [[0; 2]; 800];
        let mut step = 0;
        let start = std::time::Instant::now();
        loop {
            let count = output.read(&mut block[..[1, 113, 7, 799, 800][step % 5]]);
            result.extend_from_slice(&block[..count]);
            step += 1;
            if output.status() == Status::Ended {
                return result;
            }
            assert_ne!(output.status(), Status::Failed);
            assert!(start.elapsed() < std::time::Duration::from_secs(2));
            if count == 0 {
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
        }
    }
    #[test]
    fn output_clock_pause_gain_replacement_and_end_preserve_the_complete_stream() {
        let (_temporary, package) = package(zip::CompressionMethod::Stored, true);
        let budget = memory_budget::Budget::new(HOST_BYTES + 6 * 1048576);
        let service = Service::new(
            factory(20000, false),
            &package,
            true,
            budget.clone(),
            6 * 1048576,
        )
        .unwrap();
        drop(package);
        let output = service.output();
        for bad in [
            "../a.mp3",
            "/a.mp3",
            "https://example.invalid/a",
            "a\\b",
            "C:a",
        ] {
            assert!(service.play(bad).is_err());
        }
        assert_eq!(output.status(), Status::Stopped);
        service.play("a.mp3").unwrap();
        let baseline = drain(&output);
        assert_eq!(baseline.len(), 20000 * 48000 / 44100);
        assert_eq!(output.played_frames(), baseline.len() as u64);
        assert!(baseline
            .iter()
            .any(|frame| frame[0] > 512 && frame[1] < -512));
        service.play("a.mp3").unwrap();
        wait(|| output.queued_frames() == QUEUE_FRAMES);
        service.pause(true).unwrap();
        let mut unchanged = [[123, -456]; 113];
        assert_eq!(output.read(&mut unchanged), 0);
        assert_eq!(unchanged, [[123, -456]; 113]);
        assert_eq!(output.played_frames(), 0);
        service.pause(false).unwrap();
        assert_eq!(drain(&output), baseline);
        service.volume(64).unwrap();
        assert!(service.volume(129).is_err());
        service.play("a.mp3").unwrap();
        assert_eq!(
            drain(&output),
            baseline
                .iter()
                .map(|frame| frame.map(|s| s / 2))
                .collect::<Vec<_>>()
        );
        service.volume(0).unwrap();
        service.play("a.mp3").unwrap();
        wait(|| output.queued_frames() == QUEUE_FRAMES);
        assert_eq!(output.read(&mut unchanged), 113);
        assert_eq!(unchanged, [[0; 2]; 113]);
        assert_eq!(output.played_frames(), 113);
        service.volume(128).unwrap();
        service.play("b.mp3").unwrap();
        let second = drain(&output);
        service.play("a.mp3").unwrap();
        wait(|| output.queued_frames() == QUEUE_FRAMES);
        service.play("b.mp3").unwrap();
        assert_eq!(drain(&output), second);
        for n in 0..100 {
            service
                .play(if n % 2 == 0 { "a.mp3" } else { "b.mp3" })
                .unwrap();
        }
        assert_eq!(drain(&output), second);
        service.stop().unwrap();
        assert_eq!(output.status(), Status::Stopped);
        drop(service);
        assert_eq!(budget.used(), HOST_BYTES);
        assert_eq!(output.read(&mut unchanged), 0);
        drop(output);
        assert_eq!(budget.used(), 0);
    }
    #[test]
    fn permissions_verification_resources_and_decoder_errors_fail_without_audio() {
        let (_temporary, package) = package(zip::CompressionMethod::Stored, false);
        let budget = memory_budget::Budget::new(HOST_BYTES + 6 * 1048576);
        let factory = factory(20000, false);
        assert!(Service::new(
            factory.clone(),
            &package,
            false,
            budget.clone(),
            6 * 1048576
        )
        .is_err());
        assert!(
            Service::new(factory.clone(), &package, true, budget.clone(), 6 * 1048576).is_err()
        );
        assert_eq!(budget.used(), 0);
        let (_temporary, package) = self::package(zip::CompressionMethod::Stored, true);
        let service =
            Service::new(factory.clone(), &package, true, budget.clone(), 6 * 1048576).unwrap();
        let output = service.output();
        for bad in ["missing.mp3", "bad.mp3"] {
            service.play(bad).unwrap();
            wait(|| output.status() == Status::Failed);
            assert!(service.error().is_some());
            assert_eq!(output.read(&mut [[123; 2]; 1]), 0);
            assert_eq!(budget.used(), HOST_BYTES);
        }
        service.play("a.mp3").unwrap();
        assert!(!drain(&output).is_empty());
        drop(service);
        drop(output);
        assert_eq!(budget.used(), 0);
        let small = memory_budget::Budget::new(HOST_BYTES + 6 * 1048576 - 1);
        let limited =
            Service::new(factory.clone(), &package, true, small.clone(), 6 * 1048576).unwrap();
        limited.play("a.mp3").unwrap();
        wait(|| limited.output().status() == Status::Failed);
        assert!(limited.error().unwrap().contains("memory budget exceeded"));
        assert_eq!(small.used(), HOST_BYTES);
        drop(limited);
        assert_eq!(small.used(), 0);
        let (_temporary, compressed) = self::package(zip::CompressionMethod::Zstd, true);
        let service =
            Service::new(factory, &compressed, true, budget.clone(), 6 * 1048576).unwrap();
        service.play("a.mp3").unwrap();
        wait(|| service.output().status() == Status::Failed);
        assert!(service
            .error()
            .unwrap()
            .contains("decompressor memory accounting"));
    }
    #[test]
    fn stop_interrupts_a_bounded_decoder_and_callback_reads_never_wait_for_control_lock() {
        let (_temporary, package) = package(zip::CompressionMethod::Stored, true);
        let budget = memory_budget::Budget::new(HOST_BYTES + 6 * 1048576);
        let service = Service::new(
            factory(20000, true),
            &package,
            true,
            budget.clone(),
            6 * 1048576,
        )
        .unwrap();
        let output = service.output();
        service.play("a.mp3").unwrap();
        wait(|| output.status() == Status::Failed);
        assert!(service.error().unwrap().contains("interrupt"));
        service.play("a.mp3").unwrap();
        wait(|| budget.used() > HOST_BYTES);
        let locked = output.0.state.lock().unwrap();
        let reader = output.clone();
        let (send, receive) = std::sync::mpsc::channel();
        let test = std::thread::spawn(move || {
            send.send(reader.read(&mut [[123; 2]; 1])).unwrap();
        });
        assert_eq!(
            receive
                .recv_timeout(std::time::Duration::from_millis(200))
                .unwrap(),
            0
        );
        drop(locked);
        test.join().unwrap();
        let start = std::time::Instant::now();
        drop(service);
        assert!(start.elapsed() < std::time::Duration::from_secs(1));
        assert_eq!(budget.used(), HOST_BYTES);
        drop(output);
        assert_eq!(budget.used(), 0);
    }
}
