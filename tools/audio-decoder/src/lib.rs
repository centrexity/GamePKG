//! Trusted host companion, never a guest dependency. No filesystem or network input.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
compile_error!("build this companion for wasm32-unknown-unknown");
use std::{
    io::{self, Read, Seek, SeekFrom},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
const MAX_INPUT: usize = 8 * 1024 * 1024;
const MAX_VALUES: usize = 1600;
const WORK_BYTES: usize = 512 * 1024;
struct Input {
    bytes: Vec<u8>,
    at: u64,
    remaining: Arc<AtomicUsize>,
}
impl Read for Input {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.at == self.bytes.len() as u64 {
            return Ok(0);
        }
        let remaining = self.remaining.load(Ordering::Relaxed);
        if remaining == 0 {
            return Err(io::Error::other("encoded audio work budget exceeded"));
        }
        let count = out
            .len()
            .min(self.bytes.len() - self.at as usize)
            .min(remaining);
        out[..count].copy_from_slice(&self.bytes[self.at as usize..self.at as usize + count]);
        self.at += count as u64;
        self.remaining.store(remaining - count, Ordering::Relaxed);
        Ok(count)
    }
}
impl Seek for Input {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(at) => Some(at),
            SeekFrom::Current(delta) => self.at.checked_add_signed(delta),
            SeekFrom::End(delta) => (self.bytes.len() as u64).checked_add_signed(delta),
        }
        .filter(|at| *at <= self.bytes.len() as u64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid audio seek"))?;
        self.at = next;
        Ok(next)
    }
}
type Reader = nanomp3::Reader<Input, f32>;
struct Context {
    input: Vec<u8>,
    reader: Option<Reader>,
    remaining: Option<Arc<AtomicUsize>>,
    pending: [f32; MAX_VALUES],
    decoded: [f32; MAX_VALUES],
    pending_at: usize,
    pending_len: usize,
    output: [i16; MAX_VALUES],
    rate: u32,
    channels: u32,
    failed: bool,
}
impl Context {
    const fn new() -> Self {
        Self {
            input: Vec::new(),
            reader: None,
            remaining: None,
            pending: [0.0; MAX_VALUES],
            decoded: [0.0; MAX_VALUES],
            pending_at: 0,
            pending_len: 0,
            output: [0; MAX_VALUES],
            rate: 0,
            channels: 0,
            failed: false,
        }
    }
    fn fail(&mut self) -> i32 {
        self.failed = true;
        self.pending.fill(0.0);
        self.pending_at = 0;
        self.pending_len = 0;
        self.output.fill(0);
        -1
    }
    fn open(&mut self, bytes: Vec<u8>) -> i32 {
        self.reader = None;
        self.rate = 0;
        self.channels = 0;
        self.pending_at = 0;
        self.pending_len = 0;
        self.failed = false;
        self.output.fill(0);
        let remaining = Arc::new(AtomicUsize::new(WORK_BYTES));
        let input = Input {
            bytes,
            at: 0,
            remaining: remaining.clone(),
        };
        self.remaining = Some(remaining);
        let Ok(mut reader) =
            Reader::with_options(input, nanomp3::Options::default().skip_scan(true))
        else {
            return self.fail();
        };
        let rate = reader.sample_rate();
        let channels = match reader.channels() {
            Some(nanomp3::Channels::Mono) => 1,
            Some(nanomp3::Channels::Stereo) => 2,
            _ => return self.fail(),
        };
        if ![8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000].contains(&rate) {
            return self.fail();
        }
        let Ok(count) = reader.read(&mut self.pending) else {
            return self.fail();
        };
        if count == 0 || count > MAX_VALUES || count % channels as usize != 0 {
            return self.fail();
        }
        self.rate = rate;
        self.channels = channels;
        self.pending_len = count;
        self.reader = Some(reader);
        0
    }
}
static CONTEXT: Mutex<Context> = Mutex::new(Context::new());

// These exports expose offsets only within the companion's own Wasm memory.
// The host validates all returned offsets, lengths and types; no guest sees them.
#[no_mangle]
pub extern "C" fn decoder_reserve(length: u32) -> u32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return 0;
    };
    *context = Context::new();
    if length == 0 || length as usize > MAX_INPUT {
        return 0;
    }
    context.input.resize(length as usize, 0);
    context.input.as_mut_ptr() as usize as u32
}
#[no_mangle]
pub extern "C" fn decoder_open() -> i32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return -1;
    };
    let bytes = std::mem::take(&mut context.input);
    if bytes.is_empty() {
        return context.fail();
    }
    context.open(bytes)
}
#[no_mangle]
pub extern "C" fn decoder_read(count: u32) -> i32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return -1;
    };
    if context.failed
        || context.reader.is_none()
        || count == 0
        || count as usize > MAX_VALUES
        || !count.is_multiple_of(context.channels)
    {
        return -1;
    }
    context
        .remaining
        .as_ref()
        .unwrap()
        .store(WORK_BYTES, Ordering::Relaxed);
    let Context {
        reader,
        pending,
        pending_at,
        pending_len,
        output,
        decoded,
        ..
    } = &mut *context;
    output.fill(0);
    let used = (*pending_len - *pending_at).min(count as usize);
    decoded[..used].copy_from_slice(&pending[*pending_at..*pending_at + used]);
    *pending_at += used;
    let mut written = used;
    if used < count as usize {
        match reader
            .as_mut()
            .unwrap()
            .read(&mut decoded[used..count as usize])
        {
            Ok(n) => written += n,
            Err(_) => return context.fail(),
        }
    }
    for index in 0..written {
        // Match SDL's float-to-S16 nearest-even conversion at exact half values.
        output[index] = (decoded[index] * 32768.0)
            .round_ties_even()
            .clamp(-32768.0, 32767.0) as i16;
    }
    if !written.is_multiple_of(context.channels as usize) {
        return context.fail();
    }
    written as i32
}
#[no_mangle]
pub extern "C" fn decoder_output() -> u32 {
    CONTEXT
        .lock()
        .map_or(0, |c| c.output.as_ptr() as usize as u32)
}
#[no_mangle]
pub extern "C" fn decoder_rate() -> u32 {
    CONTEXT.lock().map_or(0, |c| c.rate)
}
#[no_mangle]
pub extern "C" fn decoder_channels() -> u32 {
    CONTEXT.lock().map_or(0, |c| c.channels)
}
#[no_mangle]
pub extern "C" fn decoder_work() -> u32 {
    CONTEXT.lock().map_or(0, |c| {
        c.remaining
            .as_ref()
            .map_or(0, |b| WORK_BYTES - b.load(Ordering::Relaxed)) as u32
    })
}
#[no_mangle]
pub extern "C" fn decoder_rewind() -> i32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return -1;
    };
    let Some(reader) = context.reader.take() else {
        return -1;
    };
    context.open(reader.into_inner().bytes)
}
