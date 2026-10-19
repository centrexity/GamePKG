//! Host-only, import-free music decoder companion. Games receive controls, never the decoder.
use anyhow::{bail, ensure, Context, Result};
use std::io::Read;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use wasmtime::{
    Config, Engine, ExternType, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc, UpdateDeadline,
};
const MAX_INPUT: usize = 8 * 1024 * 1024;
const MAX_VALUES: usize = 1600;
const MAX_MEMORY: usize = 16 * 1024 * 1024;
const WORK_BYTES: u32 = 512 * 1024;
static BUNDLED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/audio-decoder.wasm"));
struct DecoderData {
    limits: StoreLimits,
    expires: std::time::Instant,
}
fn check_deadline(data: &DecoderData) -> Result<UpdateDeadline> {
    ensure!(
        std::time::Instant::now() < data.expires,
        "audio decode interrupted: time limit exceeded"
    );
    Ok(UpdateDeadline::Continue(1))
}
fn arm(store: &mut Store<DecoderData>) {
    store.data_mut().expires = std::time::Instant::now() + std::time::Duration::from_millis(100);
    store.set_epoch_deadline(1);
}
fn configure(store: &mut Store<DecoderData>) {
    store.limiter(|data| &mut data.limits);
    store.epoch_deadline_callback(|context| check_deadline(context.data()));
    arm(store);
}
struct Shared {
    engine: Engine,
    module: Module,
    memory_maximum: usize,
    stop: Arc<AtomicBool>,
    clock: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Shared {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(clock) = self.clock.take() {
            let _ = clock.join();
        }
    }
}
#[derive(Clone)]
pub struct Factory(Arc<Shared>);
#[derive(Clone, Copy)]
pub enum Output {
    Native,
    Stereo(u32),
    OriginalMixer(u32),
}
impl Factory {
    /// Uses bytes embedded by the maintainer build. No runtime file, URL,
    /// Python interpreter or guest-selected module is consulted.
    pub fn bundled() -> Result<Self> {
        ensure!(
            !BUNDLED.is_empty(),
            "this app was built without the audio decoder companion"
        );
        let factory = Self::new(BUNDLED)?;
        factory.validate_companion()?;
        Ok(factory)
    }
    fn validate_companion(&self) -> Result<()> {
        ensure!(
            self.maximum_memory_bytes() <= 6 * 1048576,
            "bundled audio decoder must declare at most 6 MiB of memory"
        );
        for (name, parameters) in [
            ("decoder_reserve", 1),
            ("decoder_open", 0),
            ("decoder_read", 1),
            ("decoder_output", 0),
            ("decoder_rate", 0),
            ("decoder_channels", 0),
            ("decoder_work", 0),
            ("decoder_rewind", 0),
            ("decoder_set_output_rate", 1),
            ("decoder_set_mixer_rate", 1),
        ] {
            let function = self
                .0
                .module
                .exports()
                .find_map(|export| {
                    if export.name() == name {
                        if let ExternType::Func(function) = export.ty() {
                            return Some(function);
                        }
                    }
                    None
                })
                .with_context(|| format!("bundled decoder export missing: {name}"))?;
            ensure!(
                function.params().len() == parameters
                    && function
                        .params()
                        .all(|ty| matches!(ty, wasmtime::ValType::I32))
                    && function.results().len() == 1
                    && function
                        .results()
                        .all(|ty| matches!(ty, wasmtime::ValType::I32)),
                "invalid bundled decoder signature: {name}"
            );
        }
        Ok(())
    }
    pub fn bundled_digest() -> Option<String> {
        (!BUNDLED.is_empty()).then(|| crate::verify::digest(BUNDLED))
    }
    pub fn new(wasm: &[u8]) -> Result<Self> {
        ensure!(wasm.len() <= 4 * 1024 * 1024, "decoder module too large");
        let mut config = Config::new();
        config.epoch_interruption(true).max_wasm_stack(512 * 1024);
        let engine = Engine::new(&config)?;
        let module = Module::new(&engine, wasm)?;
        ensure!(
            module.imports().next().is_none(),
            "decoder companion must not import host capabilities"
        );
        let memory_type = module
            .exports()
            .find_map(|export| match export.ty() {
                ExternType::Memory(ty) if export.name() == "memory" => Some(ty),
                _ => None,
            })
            .context("decoder memory export missing")?;
        ensure!(
            !memory_type.is_64()
                && !memory_type.is_shared()
                && memory_type.maximum().is_some_and(|pages| pages <= 256),
            "decoder must declare a bounded 32-bit memory"
        );
        let memory_maximum = memory_type.maximum().unwrap() as usize * 65536;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread_engine = engine.clone();
        let clock = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(5));
                thread_engine.increment_epoch();
            }
        });
        Ok(Self(Arc::new(Shared {
            engine,
            module,
            memory_maximum,
            stop,
            clock: Some(clock),
        })))
    }
    pub fn maximum_memory_bytes(&self) -> usize {
        self.0.memory_maximum
    }
    pub fn open(&self, input: &[u8]) -> Result<Stream> {
        self.open_reader(&mut &input[..], input.len(), Output::Native, MAX_MEMORY)
    }
    pub fn open_resampled(&self, input: &[u8], rate: u32) -> Result<Stream> {
        self.open_reader(
            &mut &input[..],
            input.len(),
            Output::Stereo(rate),
            MAX_MEMORY,
        )
    }
    pub fn open_original_mixer(&self, input: &[u8], rate: u32) -> Result<Stream> {
        self.open_reader(
            &mut &input[..],
            input.len(),
            Output::OriginalMixer(rate),
            MAX_MEMORY,
        )
    }
    /// Host-only exact-length reader. Input goes directly into companion memory
    /// in at most 64 KiB reads; no second complete encoded buffer is allocated.
    /// The caller reserves this memory from the game's shared hard budget before
    /// opening. Reading an archive is trusted host work, not a guest capability.
    pub fn open_reader<R: Read + ?Sized>(
        &self,
        input: &mut R,
        encoded_len: usize,
        output: Output,
        memory_budget: usize,
    ) -> Result<Stream> {
        let (output_rate, mixer) = match output {
            Output::Native => (None, false),
            Output::Stereo(rate) => (Some(rate), false),
            Output::OriginalMixer(rate) => (Some(rate), true),
        };
        if let Some(rate) = output_rate {
            ensure!([44100, 48000].contains(&rate), "invalid music output rate");
        }
        ensure!(
            encoded_len > 0 && encoded_len <= MAX_INPUT,
            "encoded music must be 1 byte to 8 MiB"
        );
        ensure!(
            (65536..=MAX_MEMORY).contains(&memory_budget),
            "invalid decoder memory budget"
        );
        let limits = StoreLimitsBuilder::new()
            .memory_size(memory_budget)
            .memories(1)
            .instances(1)
            .tables(1)
            .table_elements(4096)
            .build();
        let mut store = Store::new(
            &self.0.engine,
            DecoderData {
                limits,
                expires: std::time::Instant::now(),
            },
        );
        configure(&mut store);
        let instance = Instance::new(&mut store, &self.0.module, &[])?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .context("decoder memory missing")?;
        let reserve = instance.get_typed_func::<u32, u32>(&mut store, "decoder_reserve")?;
        let open = instance.get_typed_func::<(), i32>(&mut store, "decoder_open")?;
        let read = instance.get_typed_func::<u32, i32>(&mut store, "decoder_read")?;
        let output = instance.get_typed_func::<(), u32>(&mut store, "decoder_output")?;
        let rate = instance.get_typed_func::<(), u32>(&mut store, "decoder_rate")?;
        let channels = instance.get_typed_func::<(), u32>(&mut store, "decoder_channels")?;
        let work = instance.get_typed_func::<(), u32>(&mut store, "decoder_work")?;
        let rewind = instance.get_typed_func::<(), i32>(&mut store, "decoder_rewind")?;
        arm(&mut store);
        let pointer = reserve.call(&mut store, encoded_len as u32)?;
        ensure!(pointer != 0, "decoder input allocation rejected");
        let start = pointer as usize;
        let end = start
            .checked_add(encoded_len)
            .context("decoder input offset overflow")?;
        let destination = memory
            .data_mut(&mut store)
            .get_mut(start..end)
            .context("decoder input is outside its memory")?;
        for chunk in destination.chunks_mut(65536) {
            input
                .read_exact(chunk)
                .context("encoded music read failed")?;
        }
        let mut extra = [0u8; 1];
        ensure!(
            input.read(&mut extra)? == 0,
            "encoded music exceeds declared length"
        );
        arm(&mut store);
        ensure!(
            open.call(&mut store, ())? == 0,
            "invalid or over-budget MPEG audio"
        );
        if let Some(rate) = output_rate {
            let configure = instance.get_typed_func::<u32, i32>(
                &mut store,
                if mixer {
                    "decoder_set_mixer_rate"
                } else {
                    "decoder_set_output_rate"
                },
            )?;
            ensure!(
                configure.call(&mut store, rate)? == 0,
                "music conversion rejected"
            );
        }
        let rate = rate.call(&mut store, ())?;
        let channels = channels.call(&mut store, ())?;
        ensure!(
            [8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000].contains(&rate)
                && matches!(channels, 1 | 2),
            "invalid decoded audio format"
        );
        if let Some(expected) = output_rate {
            ensure!(
                rate == expected && channels == 2,
                "music conversion format mismatch"
            );
        }
        ensure!(
            work.call(&mut store, ())? <= WORK_BYTES,
            "decoder exceeded encoded read budget"
        );
        Ok(Stream {
            _factory: self.clone(),
            store,
            memory,
            read,
            output,
            work,
            rewind,
            rate,
            channels,
            failed: false,
            memory_budget,
            scratch: [0; MAX_VALUES * 2],
            _reservation: None,
        })
    }
    /// Charge the decoder's entire memory ceiling before loading or decoding.
    /// Keep the charge until after its Wasm store has been destroyed.
    pub fn open_budgeted<R: Read + ?Sized>(
        &self,
        input: &mut R,
        encoded_len: usize,
        output: Output,
        budget: &crate::memory_budget::Budget,
        memory_bytes: usize,
    ) -> Result<Stream> {
        let reservation = budget.reserve(memory_bytes as u64)?;
        let mut stream = self.open_reader(input, encoded_len, output, memory_bytes)?;
        stream._reservation = Some(reservation);
        Ok(stream)
    }
}
pub struct Stream {
    _factory: Factory,
    store: Store<DecoderData>,
    memory: Memory,
    read: TypedFunc<u32, i32>,
    output: TypedFunc<(), u32>,
    work: TypedFunc<(), u32>,
    rewind: TypedFunc<(), i32>,
    rate: u32,
    channels: u32,
    failed: bool,
    memory_budget: usize,
    scratch: [u8; MAX_VALUES * 2],
    // Fields drop in declaration order: release only after the store and buffers.
    _reservation: Option<crate::memory_budget::Lease>,
}
impl Stream {
    pub fn sample_rate(&self) -> u32 {
        self.rate
    }
    pub fn channels(&self) -> u32 {
        self.channels
    }
    pub fn memory_bytes(&self) -> usize {
        self.memory.data_size(&self.store)
    }
    pub fn memory_limit_bytes(&self) -> usize {
        self.memory_budget
    }
    pub fn read(&mut self, out: &mut [i16]) -> Result<usize> {
        ensure!(
            !out.is_empty()
                && out.len() <= MAX_VALUES
                && out.len().is_multiple_of(self.channels as usize),
            "invalid decoded output buffer"
        );
        ensure!(!self.failed, "decoder is stopped after an error");
        arm(&mut self.store);
        let result = (|| -> Result<usize> {
            let count = self.read.call(&mut self.store, out.len() as u32)?;
            ensure!(
                count >= 0
                    && count as usize <= out.len()
                    && (count as usize).is_multiple_of(self.channels as usize),
                "invalid or failed decoded frame"
            );
            ensure!(
                self.work.call(&mut self.store, ())? <= WORK_BYTES,
                "decoder exceeded encoded read budget"
            );
            if count == 0 {
                return Ok(0);
            }
            let pointer = self.output.call(&mut self.store, ())? as usize;
            let count = count as usize;
            self.memory
                .read(&self.store, pointer, &mut self.scratch[..count * 2])?;
            for (sample, bytes) in out
                .iter_mut()
                .zip(self.scratch[..count * 2].as_chunks::<2>().0.iter())
            {
                *sample = i16::from_le_bytes([bytes[0], bytes[1]]);
            }
            Ok(count)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    pub fn rewind(&mut self) -> Result<()> {
        if self.failed {
            bail!("decoder is stopped after an error");
        }
        arm(&mut self.store);
        match self.rewind.call(&mut self.store, ()) {
            Ok(0) => Ok(()),
            result => {
                self.failed = true;
                bail!("decoder rewind failed: {result:?}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_validation_rejects_missing_mistyped_and_oversized_companions() {
        for (module, message) in [
            ("(module(memory(export \"memory\")1 96))", "export missing"),
            ("(module(memory(export \"memory\")1 96)(func(export \"decoder_reserve\")(param i64)(result i32)i32.const 1))", "invalid bundled decoder signature"),
            ("(module(memory(export \"memory\")1 97))", "at most 6 MiB"),
        ] {
            let factory = super::Factory::new(&wat::parse_str(module).unwrap()).unwrap();
            assert!(factory.validate_companion().unwrap_err().to_string().contains(message));
        }
    }
    #[test]
    fn bundled_factory_validates_embedded_bytes_or_reports_unavailable() {
        if super::BUNDLED.is_empty() {
            assert!(super::Factory::bundled().is_err());
            assert!(super::Factory::bundled_digest().is_none());
        } else {
            let factory = super::Factory::bundled().unwrap();
            assert!(factory.maximum_memory_bytes() <= 6 * 1048576);
            assert_eq!(
                super::Factory::bundled_digest().unwrap(),
                crate::verify::digest(super::BUNDLED)
            );
        }
    }
    use super::*;
    #[test]
    fn first_delayed_epoch_observes_the_real_deadline_without_waiting_twenty_more_ticks() {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config).unwrap();
        let module = Module::new(
            &engine,
            wat::parse_str("(module(func(export \"spin\")(loop br 0)))").unwrap(),
        )
        .unwrap();
        let mut store = Store::new(
            &engine,
            DecoderData {
                limits: StoreLimits::default(),
                expires: std::time::Instant::now(),
            },
        );
        configure(&mut store);
        let ticks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (send, receive) = std::sync::mpsc::channel();
        let checked = seen.clone();
        let clock_ticks = ticks.clone();
        store.epoch_deadline_callback(move |context| {
            let result = check_deadline(context.data());
            if result.is_err() {
                checked.store(clock_ticks.load(Ordering::SeqCst), Ordering::SeqCst);
                let _ = send.send(());
            }
            result
        });
        let spin = Instance::new(&mut store, &module, &[])
            .unwrap()
            .get_typed_func::<(), ()>(&mut store, "spin")
            .unwrap();
        arm(&mut store);
        let clock = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(125));
            ticks.store(1, Ordering::SeqCst);
            engine.increment_epoch();
            if receive
                .recv_timeout(std::time::Duration::from_millis(500))
                .is_err()
            {
                // Release a regressed nominal-tick implementation so the test
                // fails instead of leaving an infinite Wasm call behind.
                ticks.store(21, Ordering::SeqCst);
                for _ in 0..20 {
                    engine.increment_epoch();
                }
            }
        });
        assert!(spin.call(&mut store, ()).is_err());
        clock.join().unwrap();
        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }
}
