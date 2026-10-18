//! Host-only, import-free music decoder companion. Not exposed to game source yet.
use anyhow::{bail, ensure, Context, Result};
use std::io::Read;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use wasmtime::{
    Config, Engine, ExternType, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};
const MAX_INPUT: usize = 8 * 1024 * 1024;
const MAX_VALUES: usize = 1600;
const MAX_MEMORY: usize = 16 * 1024 * 1024;
const WORK_BYTES: u32 = 512 * 1024;
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
        let mut store = Store::new(&self.0.engine, limits);
        store.limiter(|limits| limits);
        store.set_epoch_deadline(20);
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
        store.set_epoch_deadline(20);
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
        store.set_epoch_deadline(20);
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
    store: Store<StoreLimits>,
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
        self.store.set_epoch_deadline(20);
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
                .zip(self.scratch[..count * 2].chunks_exact(2))
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
        self.store.set_epoch_deadline(20);
        match self.rewind.call(&mut self.store, ()) {
            Ok(0) => Ok(()),
            result => {
                self.failed = true;
                bail!("decoder rewind failed: {result:?}");
            }
        }
    }
}
