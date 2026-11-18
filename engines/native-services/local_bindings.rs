//! Trusted bootstrap registry and exact local ABI. Included only after source
//! admission; memory/thread calls stay in the CONFINED child, not the broker.
#[path = "local_runtime.rs"]
pub mod runtime;
use runtime::{Error, Result, Runtime};
use std::sync::Mutex;
static LOCAL: Mutex<Option<Runtime>> = Mutex::new(None);
const MAX: usize = 1024 * 1024;
pub fn install(runtime: Runtime) -> Result<()> {
    let mut current = LOCAL.lock().map_err(|_| Error::Failed)?;
    if current.is_some() {
        return Err(Error::Denied);
    }
    *current = Some(runtime);
    Ok(())
}
pub fn shutdown() {
    if let Ok(mut current) = LOCAL.lock() {
        current.take();
    }
}
struct Read<'a>(&'a [u8]);
impl Read<'_> {
    fn u32(&mut self) -> Result<u32> {
        let bytes = self.0.get(..4).ok_or(Error::Invalid)?;
        self.0 = &self.0[4..];
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        let bytes = self.0.get(..8).ok_or(Error::Invalid)?;
        self.0 = &self.0[8..];
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }
    fn end(&self) -> Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
    fn boolean(&mut self) -> Result<bool> {
        match self.u32()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::Invalid),
        }
    }
}
pub fn call(operation: u32, bytes: &[u8], capacity: usize) -> Result<Vec<u8>> {
    if bytes.len() > MAX || capacity > MAX {
        return Err(Error::Invalid);
    }
    let runtime = LOCAL
        .lock()
        .map_err(|_| Error::Failed)?
        .as_ref()
        .cloned()
        .ok_or(Error::Denied)?;
    let mut r = Read(bytes);
    let output = match operation {
        0 => {
            r.end()?;
            1u32.to_le_bytes().to_vec()
        }
        1 => {
            r.end()?;
            runtime
                .windows()?
                .into_iter()
                .flat_map(|(id, size)| [id.to_le_bytes(), size.to_le_bytes()].concat())
                .collect()
        }
        2 | 3 => {
            let id = r.u64()?;
            let offset = r.u64()?;
            let bytes = r.u64()?;
            let mode = r.u32()?;
            r.end()?;
            if operation == 2 {
                runtime.commit(id, offset, bytes, mode)?;
            } else {
                runtime.protect(id, offset, bytes, mode)?;
            }
            Vec::new()
        }
        4 => {
            let id = r.u64()?;
            let offset = r.u64()?;
            let bytes = r.u64()?;
            r.end()?;
            runtime.decommit(id, offset, bytes)?;
            Vec::new()
        }
        5 => {
            let id = r.u64()?;
            let offset = r.u64()?;
            let length = r.u32()? as usize;
            r.end()?;
            if length > capacity || length > MAX {
                return Err(Error::Invalid);
            }
            let mut output = vec![0; length];
            runtime.read(id, offset, &mut output)?;
            output
        }
        6 => {
            let id = r.u64()?;
            let offset = r.u64()?;
            runtime.write(id, offset, r.0)?;
            Vec::new()
        }
        7 => {
            let source = r.u64()?;
            let source_offset = r.u64()?;
            let target = r.u64()?;
            let target_offset = r.u64()?;
            let length = r.u64()?;
            r.end()?;
            runtime.copy(source, source_offset, target, target_offset, length)?;
            Vec::new()
        }
        8 => {
            let window = r.u64()?;
            let offset = r.u64()?;
            let bytes = r.u64()?;
            let value = u8::try_from(r.u64()?).map_err(|_| Error::Invalid)?;
            r.end()?;
            runtime.fill(window, offset, bytes, value)?;
            Vec::new()
        }
        9 | 15 => {
            let count = r.u32()? as usize;
            if !(1..=32).contains(&count) {
                return Err(Error::Invalid);
            }
            let mut segments = [[0u64; 3]; 32];
            let mut total = 0u64;
            for segment in &mut segments[..count] {
                *segment = [r.u64()?, r.u64()?, r.u64()?];
                if segment[2] == 0 {
                    return Err(Error::Invalid);
                }
                total = total.checked_add(segment[2]).ok_or(Error::Invalid)?;
                if total > MAX as u64 {
                    return Err(Error::Invalid);
                }
            }
            let total = total as usize;
            if operation == 9 {
                r.end()?;
                if capacity < total {
                    return Err(Error::Invalid);
                }
                let mut output = vec![0u8; total];
                runtime.read_segments(&segments[..count], &mut output)?;
                output
            } else {
                if r.0.len() != total {
                    return Err(Error::Invalid);
                }
                runtime.write_segments(&segments[..count], r.0)?;
                Vec::new()
            }
        }
        34 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            runtime.record_create(r.0)?.to_le_bytes().to_vec()
        }
        35 => {
            let record = r.u64()?;
            r.end()?;
            if !(8..=4104).contains(&capacity) {
                return Err(Error::Invalid);
            }
            let (version, bytes) = runtime.record_snapshot(record, capacity - 8)?;
            [version.to_le_bytes().as_slice(), &bytes].concat()
        }
        36 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let record = r.u64()?;
            let version = r.u64()?;
            runtime
                .record_compare_exchange(record, version, r.0)?
                .to_le_bytes()
                .to_vec()
        }
        37 => {
            let record = r.u64()?;
            r.end()?;
            runtime.record_release(record)?;
            Vec::new()
        }
        10 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let entry = r.u32()?;
            let argument = r.u64()?;
            let suspended = r.boolean()?;
            r.end()?;
            runtime
                .spawn(entry, argument, suspended)?
                .to_le_bytes()
                .to_vec()
        }
        11 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let id = r.u64()?;
            r.end()?;
            runtime.join(id)?.to_le_bytes().to_vec()
        }
        12 => {
            let id = r.u64()?;
            r.end()?;
            runtime.cancel(id)?;
            Vec::new()
        }
        13 => {
            if capacity < 4 {
                return Err(Error::Invalid);
            }
            let id = r.u64()?;
            r.end()?;
            u32::from(runtime.resume(id)?).to_le_bytes().to_vec()
        }
        14 => {
            r.end()?;
            if capacity < 12 {
                return Err(Error::Invalid);
            }
            let (id, cancelled) = runtime.current_task()?;
            [
                id.to_le_bytes().as_slice(),
                &u32::from(cancelled).to_le_bytes(),
            ]
            .concat()
        }
        16 => {
            if capacity < 4 {
                return Err(Error::Invalid);
            }
            let task = r.u64()?;
            r.end()?;
            u32::from(runtime.task_finished(task)?)
                .to_le_bytes()
                .to_vec()
        }
        17 => {
            let window = r.u64()?;
            let offset = r.u64()?;
            let pages = r.u32()?;
            let reserved = r.u32()?;
            r.end()?;
            if reserved != 0 || !(1..=256).contains(&pages) || capacity < pages as usize {
                return Err(Error::Invalid);
            }
            runtime.page_states(window, offset, pages)?
        }
        20 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let manual = r.boolean()?;
            let signaled = r.boolean()?;
            r.end()?;
            runtime.event(manual, signaled)?.to_le_bytes().to_vec()
        }
        21 => {
            r.end()?;
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            runtime.mutex()?.to_le_bytes().to_vec()
        }
        22 => {
            let id = r.u64()?;
            let signaled = r.boolean()?;
            r.end()?;
            runtime.signal(id, signaled)?;
            Vec::new()
        }
        23 => {
            let id = r.u64()?;
            let milliseconds = r.u32()?;
            r.end()?;
            runtime.wait(id, milliseconds, true)?;
            Vec::new()
        }
        24 => {
            let id = r.u64()?;
            r.end()?;
            runtime.unlock(id)?;
            Vec::new()
        }
        25 => {
            let id = r.u64()?;
            r.end()?;
            runtime.release_sync(id)?;
            Vec::new()
        }
        26 => {
            let id = r.u64()?;
            let milliseconds = r.u32()?;
            r.end()?;
            runtime.mutex_lock(id, milliseconds, true)?;
            Vec::new()
        }
        27 => {
            let id = r.u64()?;
            let milliseconds = r.u32()?;
            r.end()?;
            runtime.event_wait(id, milliseconds, true)?;
            Vec::new()
        }
        28 => {
            let id = r.u64()?;
            r.end()?;
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let value = runtime.event_state(id)?;
            [
                u32::from(value.manual).to_le_bytes(),
                u32::from(value.signaled).to_le_bytes(),
            ]
            .concat()
        }
        49 => {
            let id = r.u64()?;
            let kind = r.u32()?;
            r.end()?;
            runtime.release_sync_typed(id, kind)?;
            Vec::new()
        }
        48 => {
            if capacity < 16 {
                return Err(Error::Invalid);
            }
            let id = r.u64()?;
            r.end()?;
            let value = runtime.mutex_state(id)?;
            [
                u32::from(value.locked).to_le_bytes(),
                u32::from(value.owned_by_caller).to_le_bytes(),
                u32::from(value.abandoned).to_le_bytes(),
                value.depth.to_le_bytes(),
            ]
            .concat()
        }
        47 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let count = r.u32()? as usize;
            let all = r.boolean()?;
            let milliseconds = r.u32()?;
            if !(1..=64).contains(&count) {
                return Err(Error::Invalid);
            }
            let mut objects = Vec::with_capacity(count);
            for _ in 0..count {
                objects.push(runtime::WaitObject {
                    id: r.u64()?,
                    kind: r.u32()?,
                });
            }
            r.end()?;
            let result = runtime.wait_many(&objects, all, milliseconds, true)?;
            [
                result.index.to_le_bytes(),
                u32::from(result.abandoned).to_le_bytes(),
            ]
            .concat()
        }
        43 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let initial = r.u32()?;
            let maximum = r.u32()?;
            r.end()?;
            runtime.semaphore(initial, maximum)?.to_le_bytes().to_vec()
        }
        44 => {
            if capacity < 4 {
                return Err(Error::Invalid);
            }
            let id = r.u64()?;
            let permits = r.u32()?;
            r.end()?;
            runtime
                .semaphore_release(id, permits)?
                .to_le_bytes()
                .to_vec()
        }
        45 => {
            let id = r.u64()?;
            let milliseconds = r.u32()?;
            r.end()?;
            runtime.semaphore_wait(id, milliseconds, true)?;
            Vec::new()
        }
        46 => {
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            let id = r.u64()?;
            r.end()?;
            let value = runtime.semaphore_state(id)?;
            [value.count.to_le_bytes(), value.maximum.to_le_bytes()].concat()
        }
        30 => {
            let maximum = r.u32()? as usize;
            let depth = r.u32()? as usize;
            r.end()?;
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            runtime.mailbox(maximum, depth)?.to_le_bytes().to_vec()
        }
        31 => {
            let id = r.u64()?;
            runtime.mailbox_send(id, r.0)?;
            Vec::new()
        }
        32 => {
            let id = r.u64()?;
            let milliseconds = r.u32()?;
            r.end()?;
            runtime.mailbox_receive(id, capacity, milliseconds)?
        }
        33 => {
            let id = r.u64()?;
            r.end()?;
            runtime.mailbox_release(id)?;
            Vec::new()
        }
        38 => {
            r.end()?;
            if capacity < 24 {
                return Err(Error::Invalid);
            }
            runtime
                .performance_sample()?
                .into_iter()
                .flat_map(u64::to_le_bytes)
                .collect()
        }
        39 => {
            r.end()?;
            if capacity < 32 {
                return Err(Error::Invalid);
            }
            let sample = runtime.clock_snapshot()?;
            [
                sample.counter,
                sample.frequency,
                sample.uptime_ms,
                sample.generation,
            ]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect()
        }
        40 => {
            let deadline = r.u64()?;
            let frequency = r.u64()?;
            let generation = r.u64()?;
            r.end()?;
            if capacity < 36 {
                return Err(Error::Invalid);
            }
            let result = runtime.clock_wait(deadline, frequency, generation, true)?;
            let mut bytes = [
                result.snapshot.counter,
                result.snapshot.frequency,
                result.snapshot.uptime_ms,
                result.snapshot.generation,
            ]
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect::<Vec<_>>();
            bytes.extend(result.reason.to_le_bytes());
            bytes
        }
        42 => {
            if capacity < 4 {
                return Err(Error::Invalid);
            }
            let window = r.u64()?;
            let offset = r.u64()?;
            let length = r.u32()? as usize;
            if !(1..=65536).contains(&length) || r.0.len() != length * 2 {
                return Err(Error::Invalid);
            }
            let (expected, replacement) = r.0.split_at(length);
            u32::from(runtime.compare_exchange_memory(window, offset, expected, replacement)?)
                .to_le_bytes()
                .to_vec()
        }
        41 => {
            r.end()?;
            if capacity < 8 {
                return Err(Error::Invalid);
            }
            runtime.clock_wake()?.to_le_bytes().to_vec()
        }
        _ => return Err(Error::Unsupported),
    };
    if output.len() > capacity {
        return Err(Error::Invalid);
    }
    Ok(output)
}
/// # Safety
/// Same-process generated binding contract: buffers must be valid for their
/// stated lengths. Game source cannot form raw pointers or call FFI itself.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_local_call(
    operation: u32,
    request: *const u8,
    length: u32,
    reply: *mut u8,
    capacity: u32,
) -> i32 {
    if length as usize > MAX
        || capacity as usize > MAX
        || (request.is_null() && length != 0)
        || (reply.is_null() && capacity != 0)
    {
        return -1;
    }
    // Copy before reply so aliasing input/output in the generated client is OK.
    let bytes = if length == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(request, length as usize) }.to_vec()
    };
    match call(operation, &bytes, capacity as usize) {
        Ok(bytes) => {
            if !bytes.is_empty() {
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), reply, bytes.len());
                }
            }
            bytes.len() as i32
        }
        Err(error) => error as i32,
    }
}
