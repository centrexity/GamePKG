//! Trusted generated-binding transport, not game source. Compile as staticlib
//! or include in the launcher's generated Rust bootstrap. No external crates.
//! The trusted bootstrap binds the compatibility-layer dispatcher before game code starts.
#![deny(unsafe_op_in_unsafe_fn)]
use std::{
    fs::File,
    io::{Read, Write},
    sync::Mutex,
};
const MAX: usize = 16 * 1024 * 1024;
type Reply = std::result::Result<Vec<u8>, i32>;
type LocalDispatch = dyn FnMut(u32, &[u8], u32) -> Reply + Send;
enum Client {
    Pipe(Pipe),
    InProcess(Box<LocalDispatch>),
}
static CLIENT: Mutex<Option<Client>> = Mutex::new(None);
struct Pipe {
    input: File,
    output: File,
}

/// Host bootstrap only. Takes ownership of already validated, dedicated pipe
/// handles. On Unix these are file descriptors; on Windows real HANDLE values,
/// not CRT descriptors. Never include this declaration in game bindings.
pub fn connect(input: File, output: File) -> std::io::Result<()> {
    install(Client::Pipe(Pipe { input, output }))
}

/// Trusted bootstrap only. Install the compatibility layer's dispatcher inside
/// this executable. The callback must not call back into the generated client.
pub fn connect_local(
    dispatch: impl FnMut(u32, &[u8], u32) -> Reply + Send + 'static,
) -> std::io::Result<()> {
    install(Client::InProcess(Box::new(dispatch)))
}

fn install(client: Client) -> std::io::Result<()> {
    let mut slot = CLIENT
        .lock()
        .map_err(|_| std::io::Error::other("port client lock poisoned"))?;
    if slot.is_some() {
        return Err(std::io::Error::other("port client already bound"));
    }
    *slot = Some(client);
    Ok(())
}
/// Trusted teardown only. Drops the active connection and revokes the binding.
pub fn disconnect() {
    if let Ok(mut slot) = CLIENT.lock() {
        *slot = None;
    }
}
impl Pipe {
    fn call(&mut self, operation: u32, request: &[u8], capacity: u32) -> std::io::Result<Reply> {
        self.output.write_all(b"GPH1")?;
        self.output.write_all(&operation.to_le_bytes())?;
        self.output
            .write_all(&(request.len() as u32).to_le_bytes())?;
        self.output.write_all(&capacity.to_le_bytes())?;
        self.output.write_all(request)?;
        self.output.flush()?;
        let mut header = [0; 8];
        self.input.read_exact(&mut header)?;
        let status = i32::from_le_bytes(header[..4].try_into().unwrap());
        let len = u32::from_le_bytes(header[4..].try_into().unwrap());
        if len > capacity
            || len as usize > MAX
            || (status != 0 && len != 0)
            || !(-6..=0).contains(&status)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid host response",
            ));
        }
        let mut reply = vec![0; len as usize];
        self.input.read_exact(&mut reply)?;
        Ok(if status == 0 { Ok(reply) } else { Err(status) })
    }
}

impl Client {
    fn call(&mut self, operation: u32, request: &[u8], capacity: u32) -> std::io::Result<Reply> {
        let reply = match self {
            Self::Pipe(pipe) => pipe.call(operation, request, capacity)?,
            Self::InProcess(dispatch) => dispatch(operation, request, capacity),
        };
        let valid = match &reply {
            Ok(bytes) => bytes.len() <= capacity as usize,
            Err(status) => (-6..=-1).contains(status),
        };
        if !valid {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid service response",
            ));
        }
        Ok(reply)
    }
}

/// Native C/Rust trampoline. Valid buffers are a trusted-binding contract;
/// untrusted pointers must never be accepted in the separate host process.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_port_call(
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
    // Copy first so a game may safely reuse one buffer for request and reply.
    let bytes = if length == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(request, length as usize) }.to_vec()
    };
    let Ok(mut slot) = CLIENT.lock() else {
        return -3;
    };
    let Some(client) = slot.as_mut() else {
        return -4;
    };
    match client.call(operation, &bytes, capacity) {
        Ok(Ok(bytes)) => {
            if !bytes.is_empty() {
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), reply, bytes.len()) };
            }
            bytes.len() as i32
        }
        Ok(Err(status)) => status,
        Err(_) => {
            *slot = None;
            -3
        }
    }
}

/// Local trusted text validator. Pointer validity belongs to the generated
/// binding, just like gamepkg_port_call; no game pointer enters the host process.
/// This pure value operation has no filesystem, environment or device access.
/// # Safety
/// Nonempty input must refer to a readable, immutable allocation for this call.
#[no_mangle]
pub unsafe extern "C" fn gamepkg_port_utf8_validate(
    bytes: *const u8,
    length: u32,
    mode: u32,
) -> i32 {
    if mode > 1 || length as usize > MAX || (bytes.is_null() && length != 0) {
        return -1;
    }
    let bytes = if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, length as usize) }
    };
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return -1,
    };
    if mode == 1
        && (text.is_empty()
            || text == "."
            || text == ".."
            || text.contains(['/', '\\'])
            || text.chars().any(char::is_control))
    {
        return -1;
    }
    0
}

/// Safe dependency-free generated Rust binding. The bootstrap, not game source,
/// provides the unsafe trampoline; game source needs no extern or unsafe block.
pub fn call(operation: u32, request: &[u8], reply: &mut [u8]) -> std::result::Result<usize, i32> {
    if request.len() > MAX || reply.len() > MAX {
        return Err(-1);
    }
    let result = unsafe {
        gamepkg_port_call(
            operation,
            request.as_ptr(),
            request.len() as u32,
            reply.as_mut_ptr(),
            reply.len() as u32,
        )
    };
    if result < 0 {
        Err(result)
    } else {
        Ok(result as usize)
    }
}
/// Validate without importing unreviewed standard-library routines into a
/// game object. The immutable borrow keeps validated bytes stable until return.
pub fn text_from_utf8(bytes: &[u8]) -> std::result::Result<&str, i32> {
    validated_text(bytes, 0)
}
/// A nonempty UTF-8 component excluding separators, dot entries and controls.
/// Host portable-path validation remains authoritative for actual file access.
pub fn text_component(bytes: &[u8]) -> std::result::Result<&str, i32> {
    validated_text(bytes, 1)
}
fn validated_text(bytes: &[u8], mode: u32) -> std::result::Result<&str, i32> {
    unsafe extern "C" {
        fn gamepkg_port_utf8_validate(bytes: *const u8, length: u32, mode: u32) -> i32;
    }
    if bytes.len() > MAX {
        return Err(-1);
    }
    if unsafe { gamepkg_port_utf8_validate(bytes.as_ptr(), bytes.len() as u32, mode) } != 0 {
        return Err(-1);
    }
    // The trusted runtime validated this exact immutable allocation.
    Ok(unsafe { std::str::from_utf8_unchecked(bytes) })
}

// Safe helpers for generated Rust ports. These are supplied by GamePKG, not
// copied into signed game source by its author. No game Cargo dependency.
pub fn version() -> std::result::Result<u32, i32> {
    let mut reply = [0; 4];
    if call(0, &[], &mut reply)? != 4 {
        return Err(-3);
    }
    Ok(u32::from_le_bytes(reply))
}
/// Open a separately closeable, read-only byte range on a selected ROM grant.
pub fn file_rom_view(grant: u64, start: u64, length: u64) -> std::result::Result<u64, i32> {
    let request = [
        grant.to_le_bytes(),
        start.to_le_bytes(),
        length.to_le_bytes(),
    ]
    .concat();
    let mut reply = [0; 8];
    if call(19, &request, &mut reply)? != reply.len() {
        return Err(-3);
    }
    let id = u64::from_le_bytes(reply);
    if id == 0 {
        return Err(-3);
    }
    Ok(id)
}
pub fn file_read(handle: u64, offset: u64, reply: &mut [u8]) -> std::result::Result<usize, i32> {
    if reply.len() > MAX {
        return Err(-1);
    }
    call(
        2,
        &[
            handle.to_le_bytes().as_slice(),
            &offset.to_le_bytes(),
            &(reply.len() as u32).to_le_bytes(),
        ]
        .concat(),
        reply,
    )
}
pub fn file_size(handle: u64) -> std::result::Result<u64, i32> {
    let mut reply = [0; 8];
    if call(5, &handle.to_le_bytes(), &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn file_open(grant: u64, writable: bool, name: &str) -> std::result::Result<u64, i32> {
    file_open_with_mode(grant, u32::from(writable), name)
}
/// Open an existing private save for reading and writing without resizing it.
pub fn file_open_existing(grant: u64, name: &str) -> std::result::Result<u64, i32> {
    file_open_with_mode(grant, 2, name)
}
/// Create a new private save atomically. An existing entry is never opened.
pub fn file_create_new(grant: u64, name: &str) -> std::result::Result<u64, i32> {
    file_open_with_mode(grant, 3, name)
}
fn file_open_with_mode(grant: u64, mode: u32, name: &str) -> std::result::Result<u64, i32> {
    if name.len() > 4096 {
        return Err(-1);
    }
    let request = [
        grant.to_le_bytes().as_slice(),
        &mode.to_le_bytes(),
        name.as_bytes(),
    ]
    .concat();
    let mut reply = [0; 8];
    if call(1, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn file_write(handle: u64, offset: u64, bytes: &[u8]) -> std::result::Result<(), i32> {
    if bytes.len() > MAX - 16 {
        return Err(-1);
    }
    call(
        3,
        &[
            handle.to_le_bytes().as_slice(),
            &offset.to_le_bytes(),
            bytes,
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
pub fn file_close(handle: u64) -> std::result::Result<(), i32> {
    call(4, &handle.to_le_bytes(), &mut [])?;
    Ok(())
}
pub struct FileMetadata {
    pub length: u64,
    pub modified_seconds: i64,
    pub modified_nanoseconds: u32,
}
/// Metadata of this opened file, including after rename/unlink. No host path.
pub fn file_metadata(handle: u64) -> std::result::Result<FileMetadata, i32> {
    let mut reply = [0; 24];
    if call(15, &handle.to_le_bytes(), &mut reply)? != reply.len() {
        return Err(-3);
    }
    let nanoseconds = u32::from_le_bytes(fixed4(&reply, 16));
    if nanoseconds >= 1_000_000_000 || reply[20..] != [0; 4] {
        return Err(-3);
    }
    Ok(FileMetadata {
        length: u64::from_le_bytes(fixed8(&reply, 0)),
        modified_seconds: i64::from_le_bytes(fixed8(&reply, 8)),
        modified_nanoseconds: nanoseconds,
    })
}
pub struct SaveFileInfo {
    pub length: u64,
    /// Creation, access, write and change times in 100 ns units since 1601 UTC.
    pub times: [u64; 4],
    pub attributes: u32,
}
/// Observes a retained regular file in a private save grant.
pub fn save_file_info(handle: u64) -> std::result::Result<SaveFileInfo, i32> {
    let mut reply = [0; 48];
    if call(25, &handle.to_le_bytes(), &mut reply)? != 48 {
        return Err(-3);
    }
    let times = [
        u64::from_le_bytes(fixed8(&reply, 8)),
        u64::from_le_bytes(fixed8(&reply, 16)),
        u64::from_le_bytes(fixed8(&reply, 24)),
        u64::from_le_bytes(fixed8(&reply, 32)),
    ];
    let attributes = u32::from_le_bytes(fixed4(&reply, 40));
    if times.iter().any(|&time| time > i64::MAX as u64)
        || (attributes != 0x80 && (attributes == 0 || attributes & !0x27 != 0))
        || reply[44..] != [0; 4]
    {
        return Err(-3);
    }
    Ok(SaveFileInfo {
        length: u64::from_le_bytes(fixed8(&reply, 0)),
        times,
        attributes,
    })
}
/// Sets the portable hidden, system and archive bits on a writable save file.
pub fn save_file_set_attributes(handle: u64, attributes: u32) -> std::result::Result<(), i32> {
    if attributes & !0x26 != 0 {
        return Err(-1);
    }
    let request = [handle.to_le_bytes().as_slice(), &attributes.to_le_bytes()].concat();
    if call(26, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
/// Independent close lifetime with the same rights and grant revocation.
pub fn file_duplicate(handle: u64) -> std::result::Result<u64, i32> {
    let mut reply = [0; 8];
    if call(16, &handle.to_le_bytes(), &mut reply)? != reply.len() {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn file_sync(handle: u64) -> std::result::Result<(), i32> {
    call(12, &handle.to_le_bytes(), &mut []).map(|_| ())
}
pub fn file_resize(handle: u64, length: u64) -> std::result::Result<(), i32> {
    let request = [handle.to_le_bytes(), length.to_le_bytes()].concat();
    call(9, &request, &mut []).map(|_| ())
}
pub fn file_remove(grant: u64, name: &str) -> std::result::Result<(), i32> {
    if name.len() > 4096 {
        return Err(-1);
    }
    let request = [grant.to_le_bytes().as_slice(), name.as_bytes()].concat();
    call(8, &request, &mut []).map(|_| ())
}
/// False only for confirmed absence inside this directory capability.
pub fn file_exists(grant: u64, name: &str) -> std::result::Result<bool, i32> {
    if name.len() > 4096 {
        return Err(-1);
    }
    let request = [grant.to_le_bytes().as_slice(), name.as_bytes()].concat();
    let mut reply = [0; 4];
    if call(17, &request, &mut reply)? != 4 {
        return Err(-3);
    }
    match u32::from_le_bytes(reply) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(-3),
    }
}
pub fn directory_create(grant: u64, name: &str) -> std::result::Result<(), i32> {
    if name.len() > 4096 {
        return Err(-1);
    }
    let request = [grant.to_le_bytes().as_slice(), name.as_bytes()].concat();
    call(7, &request, &mut []).map(|_| ())
}
/// Remove only an empty directory in a writable save capability.
pub fn directory_remove(grant: u64, name: &str) -> std::result::Result<(), i32> {
    if name.len() > 4096 {
        return Err(-1);
    }
    let request = [grant.to_le_bytes().as_slice(), name.as_bytes()].concat();
    call(18, &request, &mut []).map(|_| ())
}
pub struct DirectoryEntry {
    pub name: String,
    pub directory: bool,
}
/// Private save quota and available logical bytes, unrelated to disk capacity.
pub fn save_volume(grant: u64) -> std::result::Result<[u64; 2], i32> {
    let mut reply = [0; 16];
    if call(23, &grant.to_le_bytes(), &mut reply)? != 16 {
        return Err(-3);
    }
    let limit = u64::from_le_bytes(fixed8(&reply, 0));
    let available = u64::from_le_bytes(fixed8(&reply, 8));
    if available > limit {
        return Err(-3);
    }
    Ok([limit, available])
}
/// Observe a directory in a private save grant. In create mode, create just
/// the final component if absent. The reply is 0 absent, 1 opened or 2 created.
pub fn save_directory(grant: u64, name: &str, create: bool) -> std::result::Result<u32, i32> {
    if name.len() > 4096 || (create && name.is_empty()) {
        return Err(-1);
    }
    let request = [
        grant.to_le_bytes().as_slice(),
        &u32::from(create).to_le_bytes(),
        name.as_bytes(),
    ]
    .concat();
    let mut reply = [0; 4];
    if call(24, &request, &mut reply)? != 4 {
        return Err(-3);
    }
    match (create, u32::from_le_bytes(reply)) {
        (false, state @ 0..=1) | (true, state @ 1..=2) => Ok(state),
        _ => Err(-3),
    }
}
pub fn directory_list(grant: u64, name: &str) -> std::result::Result<Vec<DirectoryEntry>, i32> {
    if name.len() > 4096 {
        return Err(-1);
    }
    let request = [grant.to_le_bytes().as_slice(), name.as_bytes()].concat();
    let mut bytes = vec![0; 4096 * (8 + 255)];
    let length = call(6, &request, &mut bytes)?;
    decode_directory_entries(&bytes[..length])
}
fn decode_directory_entries(mut remaining: &[u8]) -> std::result::Result<Vec<DirectoryEntry>, i32> {
    let mut entries = Vec::new();
    while !remaining.is_empty() {
        if remaining.len() < 8 || entries.len() >= 4096 {
            return Err(-3);
        }
        let kind = u32::from_le_bytes(fixed4(remaining, 0));
        let length = u32::from_le_bytes(fixed4(remaining, 4)) as usize;
        if !matches!(kind, 1 | 2) || !(1..=255).contains(&length) {
            return Err(-3);
        }
        let name = remaining.get(8..8 + length).ok_or(-3)?;
        let name = text_component(name).map_err(|_| -3)?;
        entries.push(DirectoryEntry {
            name: name.to_owned(),
            directory: kind == 2,
        });
        remaining = &remaining[8 + length..];
    }
    Ok(entries)
}
/// Flush the temporary source, then atomically replace a save in the same
/// capability. -6 means published but directory durability could not be proven.
pub fn file_replace(
    grant: u64,
    source: &str,
    destination: &str,
    durable: bool,
) -> std::result::Result<(), i32> {
    if source.len() > 4096 || destination.len() > 4096 {
        return Err(-1);
    }
    let mut request = Vec::new();
    request.extend(grant.to_le_bytes());
    request.extend(u32::from(durable).to_le_bytes());
    request.extend((source.len() as u32).to_le_bytes());
    request.extend(source.as_bytes());
    request.extend(destination.as_bytes());
    call(14, &request, &mut []).map(|_| ())
}
/// Emit at most 32 numeric values to this session's bounded host diagnostic
/// ring. Returns its sequence number; never opens a console, path or socket.
pub fn diagnostic(level: u32, code: u32, values: &[u64]) -> std::result::Result<u64, i32> {
    if level > 3 || values.len() > 32 {
        return Err(-1);
    }
    let mut request = Vec::with_capacity(12 + values.len() * 8);
    request.extend(level.to_le_bytes());
    request.extend(code.to_le_bytes());
    request.extend((values.len() as u32).to_le_bytes());
    for value in values {
        request.extend(value.to_le_bytes());
    }
    let mut reply = [0u8; 8];
    if call(63, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}

/// Options selected at launch. A missing value leaves the game's preference
/// in effect. Window/aspect/AA/LOD value mappings are defined by operation 67.
pub struct LaunchSettings {
    pub fullscreen: Option<bool>,
    pub widescreen: Option<bool>,
    pub render_scale: Option<u8>,
    pub antialiasing: Option<u8>,
    pub texture_lod: Option<u8>,
    pub sharpening_percent: Option<u8>,
    pub widescreen_hud: Option<bool>,
    pub menu_hud_scale: Option<u8>,
}

pub fn launch_settings() -> std::result::Result<LaunchSettings, i32> {
    let mut bytes = [0; 8];
    if call(67, &[], &mut bytes)? != bytes.len() {
        return Err(-3);
    }
    let mut options = [None; 8];
    for (index, (min, max)) in [
        (0, 1),
        (0, 1),
        (1, 4),
        (0, 2),
        (0, 4),
        (0, 100),
        (0, 1),
        (1, 4),
    ]
    .into_iter()
    .enumerate()
    {
        if bytes[index] != u8::MAX {
            if !(min..=max).contains(&bytes[index]) {
                return Err(-3);
            }
            options[index] = Some(bytes[index]);
        }
    }
    Ok(LaunchSettings {
        fullscreen: options[0].map(|value| value == 1),
        widescreen: options[1].map(|value| value == 1),
        render_scale: options[2],
        antialiasing: options[3],
        texture_lod: options[4],
        sharpening_percent: options[5],
        widescreen_hud: options[6].map(|value| value == 1),
        menu_hud_scale: options[7],
    })
}
/// Retain at most 128 KiB of exact diagnostic bytes, including non-UTF-8 data.
/// Host summaries escape control bytes; no format string or console is exposed.
pub fn diagnostic_bytes(level: u32, code: u32, bytes: &[u8]) -> std::result::Result<u64, i32> {
    if level > 3 || bytes.len() > 128 * 1024 {
        return Err(-1);
    }
    let mut request = Vec::with_capacity(12 + bytes.len());
    request.extend(level.to_le_bytes());
    request.extend(code.to_le_bytes());
    request.extend((bytes.len() as u32).to_le_bytes());
    request.extend(bytes);
    let mut reply = [0u8; 8];
    if call(64, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
/// Session-relative nanoseconds and generation used for lost-wake-free waits.
pub fn monotonic() -> std::result::Result<(u64, u64), i32> {
    let mut reply = [0; 16];
    if call(60, &[], &mut reply)? != 16 {
        return Err(-3);
    }
    Ok((
        u64::from_le_bytes(fixed8(&reply, 0)),
        u64::from_le_bytes(fixed8(&reply, 8)),
    ))
}
/// Host-owned raw counter units/frequency, boot milliseconds and monotonic-
/// advanced UTC in 100ns intervals since 1601-01-01. No native clock handle.
#[derive(Debug, PartialEq, Eq)]
pub struct ClockSample {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
    pub utc_100ns: u64,
}
pub fn clock_sample() -> std::result::Result<ClockSample, i32> {
    let mut reply = [0u8; 32];
    let length = call(65, &[], &mut reply)?;
    clock_sample_decode(&reply[..length])
}
fn clock_sample_decode(reply: &[u8]) -> std::result::Result<ClockSample, i32> {
    if reply.len() != 32 {
        return Err(-3);
    }
    let counter = u64::from_le_bytes(fixed8(reply, 0));
    let frequency = u64::from_le_bytes(fixed8(reply, 8));
    if counter > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64 {
        return Err(-3);
    }
    Ok(ClockSample {
        counter,
        frequency,
        uptime_ms: u64::from_le_bytes(fixed8(reply, 16)),
        utc_100ns: u64::from_le_bytes(fixed8(reply, 24)),
    })
}
#[derive(Debug, PartialEq, Eq)]
pub struct PerformanceSample {
    pub counter: u64,
    pub frequency: u64,
    pub uptime_ms: u64,
}
/// Read raw performance units without reading or anchoring calendar time.
pub fn performance_sample() -> std::result::Result<PerformanceSample, i32> {
    let mut reply = [0u8; 24];
    let length = call(66, &[], &mut reply)?;
    performance_sample_decode(&reply[..length])
}
/// Read the trusted child-local clock without acquiring the broker connection.
/// Same units as performance_sample; does not read or anchor calendar time.
pub fn local_performance_sample() -> std::result::Result<PerformanceSample, i32> {
    let mut reply = [0u8; 24];
    let length = local_call(38, &[], &mut reply)?;
    performance_sample_decode(&reply[..length])
}
#[derive(Debug, PartialEq, Eq)]
pub struct LocalClockState {
    pub sample: PerformanceSample,
    pub generation: u64,
}
#[derive(Debug, PartialEq, Eq)]
pub struct LocalClockWait {
    pub state: LocalClockState,
    pub reason: u32,
}
/// Raw performance-clock state for the child-local timing lane, not session ns.
pub fn local_clock_state() -> std::result::Result<LocalClockState, i32> {
    let mut reply = [0u8; 32];
    let length = local_call(39, &[], &mut reply)?;
    local_clock_state_decode(&reply[..length])
}
fn local_clock_state_decode(reply: &[u8]) -> std::result::Result<LocalClockState, i32> {
    if reply.len() != 32 {
        return Err(-3);
    }
    Ok(LocalClockState {
        sample: performance_sample_decode(&reply[..24])?,
        generation: u64::from_le_bytes(fixed8(reply, 24)),
    })
}
/// At most one second ahead, using the state's raw counter/frequency. Reason
/// 0=deadline, 1=changed generation (including a wake that precedes the wait).
pub fn local_clock_wait(
    deadline: u64,
    frequency: u64,
    generation: u64,
) -> std::result::Result<LocalClockWait, i32> {
    if deadline > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64 {
        return Err(-1);
    }
    let request = [
        deadline.to_le_bytes(),
        frequency.to_le_bytes(),
        generation.to_le_bytes(),
    ]
    .concat();
    let mut reply = [0u8; 36];
    let length = local_call(40, &request, &mut reply)?;
    local_clock_wait_decode(&reply[..length], deadline, frequency, generation)
}
fn local_clock_wait_decode(
    reply: &[u8],
    deadline: u64,
    frequency: u64,
    generation: u64,
) -> std::result::Result<LocalClockWait, i32> {
    if reply.len() != 36 {
        return Err(-3);
    }
    let state = local_clock_state_decode(&reply[..32])?;
    let reason = u32::from_le_bytes(fixed4(reply, 32));
    if state.sample.frequency != frequency
        || state.generation < generation
        || reason > 1
        || (reason == 0 && (state.generation != generation || state.sample.counter < deadline))
        || (reason == 1 && state.generation == generation)
    {
        return Err(-3);
    }
    Ok(LocalClockWait { state, reason })
}
pub fn local_clock_wake() -> std::result::Result<u64, i32> {
    let mut reply = [0u8; 8];
    if local_call(41, &[], &mut reply)? != 8 {
        return Err(-3);
    }
    let generation = u64::from_le_bytes(reply);
    if generation == 0 {
        return Err(-3);
    }
    Ok(generation)
}
fn performance_sample_decode(reply: &[u8]) -> std::result::Result<PerformanceSample, i32> {
    if reply.len() != 24 {
        return Err(-3);
    }
    let counter = u64::from_le_bytes(fixed8(reply, 0));
    let frequency = u64::from_le_bytes(fixed8(reply, 8));
    if counter > i64::MAX as u64 || frequency == 0 || frequency > i64::MAX as u64 {
        return Err(-3);
    }
    Ok(PerformanceSample {
        counter,
        frequency,
        uptime_ms: u64::from_le_bytes(fixed8(reply, 16)),
    })
}
/// Absolute session deadline, <=1 second ahead. reason 0=deadline, 1=wakeup.
pub fn wait_until(deadline: u64, generation: u64) -> std::result::Result<(u64, u64, u32), i32> {
    let request = [deadline.to_le_bytes(), generation.to_le_bytes()].concat();
    let mut reply = [0; 20];
    if call(61, &request, &mut reply)? != 20 {
        return Err(-3);
    }
    Ok((
        u64::from_le_bytes(fixed8(&reply, 0)),
        u64::from_le_bytes(fixed8(&reply, 8)),
        u32::from_le_bytes(fixed4(&reply, 16)),
    ))
}
pub fn fullscreen(enabled: bool) -> std::result::Result<(), i32> {
    window(0, u32::from(enabled))
}
pub fn cursor_visible(enabled: bool) -> std::result::Result<(), i32> {
    window(1, u32::from(enabled))
}
pub fn cursor_locked(enabled: bool) -> std::result::Result<(), i32> {
    window(2, u32::from(enabled))
}
/// Physical pixels and current flags for the game process's window.
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub focused: bool,
    pub closed: bool,
}
pub fn window_state() -> std::result::Result<WindowState, i32> {
    let mut reply = [0; 12];
    if call(11, &[], &mut reply)? != reply.len() {
        return Err(-3);
    }
    let flags = u32_at(&reply, 8);
    if flags & !7 != 0 {
        return Err(-3);
    }
    Ok(WindowState {
        width: u32_at(&reply, 0),
        height: u32_at(&reply, 4),
        fullscreen: flags & 1 != 0,
        focused: flags & 2 != 0,
        closed: flags & 4 != 0,
    })
}
fn window(command: u32, value: u32) -> std::result::Result<(), i32> {
    call(
        10,
        &[command.to_le_bytes(), value.to_le_bytes()].concat(),
        &mut [],
    )?;
    Ok(())
}
pub fn audio(samples: &[i16], channels: u32) -> std::result::Result<(), i32> {
    if !matches!(channels, 1 | 2)
        || samples.is_empty()
        || !samples.len().is_multiple_of(channels as usize)
        || samples.len() > 1600 * channels as usize
    {
        return Err(-1);
    }
    let mut request = channels.to_le_bytes().to_vec();
    for s in samples {
        request.extend(s.to_le_bytes());
    }
    call(30, &request, &mut [])?;
    Ok(())
}
pub fn network(
    operation: u32,
    request: &[u8],
    reply: &mut [u8],
) -> std::result::Result<usize, i32> {
    if operation > 13 || request.len() > 65540 || reply.len() > 65536 {
        return Err(-1);
    }
    call(
        50,
        &[operation.to_le_bytes().as_slice(), request].concat(),
        reply,
    )
}
pub fn key_held(device: u64, usage: u32) -> std::result::Result<bool, i32> {
    let mut reply = [0; 4];
    if call(
        42,
        &[device.to_le_bytes().as_slice(), &usage.to_le_bytes()].concat(),
        &mut reply,
    )? != 4
    {
        return Err(-3);
    }
    Ok(u32::from_le_bytes(reply) != 0)
}
pub fn rumble(player: u32, low: u16, high: u16, milliseconds: u32) -> std::result::Result<(), i32> {
    call(
        45,
        &[
            player.to_le_bytes(),
            u32::from(low).to_le_bytes(),
            u32::from(high).to_le_bytes(),
            milliseconds.to_le_bytes(),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputEvent {
    pub device: u64,
    pub timestamp_ns: u64,
    pub kind: u32,
    pub code: u32,
    pub x: f32,
    pub y: f32,
    pub contact: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grant {
    pub id: u64,
    pub kind: u32,
}
/// Only grants explicitly supplied to this session. Kinds: assets=1, saves=2,
/// ROM=3, disposable fixed-capacity Linux RAM cache=4, previous save=5.
/// Never host paths.
pub fn grants() -> std::result::Result<Vec<Grant>, i32> {
    let mut reply = vec![0; 48 * 16];
    let n = call(13, &[], &mut reply)?;
    if n % 16 != 0 {
        return Err(-3);
    }
    Ok(reply[..n]
        .chunks_exact(16)
        .map(|b| Grant {
            id: u64_at(b, 0),
            kind: u32_at(b, 8),
        })
        .collect())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gamepad {
    pub device: u64,
    pub timestamp_ns: u64,
    pub buttons: u32,
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    pub left_trigger: u16,
    pub right_trigger: u16,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub device: u64,
    pub timestamp_ns: u64,
    pub contact: u64,
    pub x: f32,
    pub y: f32,
}
// Fixed-width decoders retain bounds traps without formatting an unwrap error.
fn fixed2(b: &[u8], n: usize) -> [u8; 2] {
    [b[n], b[n + 1]]
}
fn fixed4(b: &[u8], n: usize) -> [u8; 4] {
    [b[n], b[n + 1], b[n + 2], b[n + 3]]
}
fn fixed8(b: &[u8], n: usize) -> [u8; 8] {
    [
        b[n],
        b[n + 1],
        b[n + 2],
        b[n + 3],
        b[n + 4],
        b[n + 5],
        b[n + 6],
        b[n + 7],
    ]
}
fn u32_at(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(fixed4(b, n))
}
fn u64_at(b: &[u8], n: usize) -> u64 {
    u64::from_le_bytes(fixed8(b, n))
}
pub fn input_events() -> std::result::Result<Vec<InputEvent>, i32> {
    let mut bytes = vec![0; 2048 * 40];
    let n = call(40, &[], &mut bytes)?;
    if n % 40 != 0 {
        return Err(-3);
    }
    Ok(bytes[..n]
        .chunks_exact(40)
        .map(|b| InputEvent {
            device: u64_at(b, 0),
            timestamp_ns: u64_at(b, 8),
            kind: u32_at(b, 16),
            code: u32_at(b, 20),
            x: f32::from_bits(u32_at(b, 24)),
            y: f32::from_bits(u32_at(b, 28)),
            contact: u64_at(b, 32),
        })
        .collect())
}
/// Logical player index 0..3. device==0 means unclaimed/disconnected.
pub fn gamepad(player: u32) -> std::result::Result<Gamepad, i32> {
    let mut b = [0; 32];
    if call(41, &player.to_le_bytes(), &mut b)? != 32 {
        return Err(-3);
    }
    let axis = |n| i16::from_le_bytes(fixed2(&b, n));
    Ok(Gamepad {
        device: u64_at(&b, 0),
        timestamp_ns: u64_at(&b, 8),
        buttons: u32_at(&b, 16),
        left_x: axis(20),
        left_y: axis(22),
        right_x: axis(24),
        right_y: axis(26),
        left_trigger: u16::from_le_bytes(fixed2(&b, 28)),
        right_trigger: u16::from_le_bytes(fixed2(&b, 30)),
    })
}
pub fn touch_contacts() -> std::result::Result<Vec<Contact>, i32> {
    let mut bytes = vec![0; 16 * 16 * 32];
    let n = call(44, &[], &mut bytes)?;
    if n % 32 != 0 {
        return Err(-3);
    }
    Ok(bytes[..n]
        .chunks_exact(32)
        .map(|b| Contact {
            device: u64_at(b, 0),
            timestamp_ns: u64_at(b, 8),
            contact: u64_at(b, 16),
            x: f32::from_bits(u32_at(b, 24)),
            y: f32::from_bits(u32_at(b, 28)),
        })
        .collect())
}
#[derive(Clone, Copy, Debug)]
pub struct ImageDraw {
    pub image: u64,
    pub rect: [f32; 4],
    pub linear: bool,
    pub shader: Option<u32>,
}
pub fn image_create(width: u32, height: u32, rgba: &[u8]) -> std::result::Result<u64, i32> {
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || rgba.len() > MAX - 8
        || u64::from(width) * u64::from(height) * 4 != rgba.len() as u64
    {
        return Err(-1);
    }
    let mut reply = [0; 8];
    let request = [width.to_le_bytes().as_slice(), &height.to_le_bytes(), rgba].concat();
    if call(20, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn image_release(handle: u64) -> std::result::Result<(), i32> {
    call(21, &handle.to_le_bytes(), &mut [])?;
    Ok(())
}
pub fn present(clear: u32, draws: &[ImageDraw]) -> std::result::Result<(), i32> {
    if draws.len() > 4096 {
        return Err(-1);
    }
    let mut request = clear.to_le_bytes().to_vec();
    request.extend((draws.len() as u32).to_le_bytes());
    for d in draws {
        request.extend(d.image.to_le_bytes());
        for value in d.rect {
            request.extend(value.to_le_bytes());
        }
        request.extend(u32::from(d.linear).to_le_bytes());
        request.extend(d.shader.unwrap_or(u32::MAX).to_le_bytes());
    }
    call(22, &request, &mut [])?;
    Ok(())
}

pub fn buffer_create(kind: u32, bytes: &[u8]) -> std::result::Result<u64, i32> {
    if kind > 2 || bytes.len() > MAX - 4 {
        return Err(-1);
    }
    let mut reply = [0; 8];
    if call(
        100,
        &[kind.to_le_bytes().as_slice(), bytes].concat(),
        &mut reply,
    )? != 8
    {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn buffer_write(handle: u64, offset: u64, bytes: &[u8]) -> std::result::Result<(), i32> {
    if bytes.len() > MAX - 16 {
        return Err(-1);
    }
    call(
        101,
        &[
            handle.to_le_bytes().as_slice(),
            &offset.to_le_bytes(),
            bytes,
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
pub fn texture_create(
    width: u32,
    height: u32,
    layers: u32,
    mips: u32,
    srgb: bool,
) -> std::result::Result<u64, i32> {
    let request = [width, height, layers, mips, u32::from(srgb)]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut reply = [0; 8];
    if call(102, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
#[derive(Clone, Copy, Debug)]
pub struct TextureRegion {
    pub mip: u32,
    pub layer: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
pub fn texture_write(
    handle: u64,
    region: TextureRegion,
    bytes: &[u8],
) -> std::result::Result<(), i32> {
    if bytes.len() > MAX - 32 {
        return Err(-1);
    }
    let mut request = handle.to_le_bytes().to_vec();
    for n in [
        region.mip,
        region.layer,
        region.x,
        region.y,
        region.width,
        region.height,
    ] {
        request.extend(n.to_le_bytes());
    }
    request.extend(bytes);
    call(103, &request, &mut [])?;
    Ok(())
}
pub fn gpu_release(handle: u64) -> std::result::Result<(), i32> {
    call(104, &handle.to_le_bytes(), &mut [])?;
    Ok(())
}
pub fn target_create(width: u32, height: u32) -> std::result::Result<u64, i32> {
    let mut reply = [0; 8];
    if call(
        106,
        &[width.to_le_bytes(), height.to_le_bytes()].concat(),
        &mut reply,
    )? != 8
    {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn target_present(target: u64) -> std::result::Result<(), i32> {
    call(109, &target.to_le_bytes(), &mut []).map(|_| ())
}
pub fn target_copy(
    source: u64,
    destination: u64,
    source_xy: [u32; 2],
    destination_xy: [u32; 2],
    size: [u32; 2],
) -> std::result::Result<(), i32> {
    let mut request = [source.to_le_bytes(), destination.to_le_bytes()].concat();
    for n in source_xy.into_iter().chain(destination_xy).chain(size) {
        request.extend(n.to_le_bytes());
    }
    call(107, &request, &mut []).map(|_| ())
}
pub fn target_clear(
    target: u64,
    mask: u32,
    rectangle: [u32; 4],
    color: u32,
    depth: f32,
) -> std::result::Result<(), i32> {
    let mut request = target.to_le_bytes().to_vec();
    for n in [mask]
        .into_iter()
        .chain(rectangle)
        .chain([color, depth.to_bits()])
    {
        request.extend(n.to_le_bytes());
    }
    call(110, &request, &mut []).map(|_| ())
}
pub fn visibility_create() -> std::result::Result<u64, i32> {
    let mut reply = [0; 8];
    if call(111, &[], &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
/// Busy means no result yet. A successful poll consumes the result and permits
/// reuse; gpu_release cancels ownership without waiting for GPU completion.
pub fn visibility_poll(query: u64) -> std::result::Result<u64, i32> {
    let mut reply = [0; 8];
    if call(113, &query.to_le_bytes(), &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
#[derive(Clone, Copy, Debug)]
pub struct TextureBinding {
    pub texture: u64,
    pub sampler: u32,
}
#[derive(Clone, Copy)]
pub struct IndexedDraw<'a> {
    pub pipeline: u32,
    pub vertex: u64,
    pub index: u64,
    pub uniform: u64,
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub scissor: [u32; 4],
    pub textures: &'a [TextureBinding],
}
#[derive(Clone, Copy)]
pub struct DrawState<'a> {
    /// Zero selects IndexedDraw.pipeline; otherwise a ready opaque variant ID.
    pub pipeline_variant: u64,
    pub blend_constant: [f32; 4],
    pub stencil_reference: u32,
    /// Empty uses legacy flags; nonzero IDs override each corresponding sampler.
    pub samplers: &'a [u64],
}
pub struct TextureCopy {
    pub source: u64,
    pub destination: u64,
    pub source_xy: [u32; 2],
    pub mip: u32,
    pub layer: u32,
    pub destination_xy: [u32; 2],
    pub size: [u32; 2],
}
pub fn target_copy_texture(copy: &TextureCopy) -> std::result::Result<(), i32> {
    let mut request = [copy.source.to_le_bytes(), copy.destination.to_le_bytes()].concat();
    for n in copy
        .source_xy
        .into_iter()
        .chain([copy.mip, copy.layer])
        .chain(copy.destination_xy)
        .chain(copy.size)
    {
        request.extend(n.to_le_bytes());
    }
    call(116, &request, &mut []).map(|_| ())
}
pub fn depth_create(width: u32, height: u32, stencil: bool) -> std::result::Result<u64, i32> {
    let mut reply = [0; 8];
    let request = [width, height, u32::from(stencil)]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    if call(118, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn target_depth(target: u64, depth: u64) -> std::result::Result<(), i32> {
    call(
        119,
        &[target.to_le_bytes(), depth.to_le_bytes()].concat(),
        &mut [],
    )
    .map(|_| ())
}
pub fn target_clear_stencil(
    target: u64,
    mask: u32,
    area: [u32; 4],
    color: u32,
    depth: f32,
    stencil: u32,
) -> std::result::Result<(), i32> {
    let mut request = target.to_le_bytes().to_vec();
    for n in [mask]
        .into_iter()
        .chain(area)
        .chain([color, depth.to_bits(), stencil])
    {
        request.extend(n.to_le_bytes());
    }
    call(120, &request, &mut []).map(|_| ())
}
pub struct Sampler {
    pub min_linear: bool,
    pub mag_linear: bool,
    pub mip_linear: bool,
    pub address: [u32; 3],
    pub lod: [f32; 2],
    pub anisotropy: u32,
}
pub fn sampler_create(s: &Sampler) -> std::result::Result<u64, i32> {
    let request = [
        u32::from(s.min_linear),
        u32::from(s.mag_linear),
        u32::from(s.mip_linear),
    ]
    .into_iter()
    .chain(s.address)
    .chain(s.lod.map(f32::to_bits))
    .chain([s.anisotropy])
    .flat_map(u32::to_le_bytes)
    .collect::<Vec<_>>();
    let mut reply = [0; 8];
    if call(122, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn surface_readback(target: u64, area: [u32; 4]) -> std::result::Result<u64, i32> {
    let mut request = target.to_le_bytes().to_vec();
    for n in area {
        request.extend(n.to_le_bytes());
    }
    let mut reply = [0; 8];
    if call(124, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn readback_poll(ticket: u64, rgba: &mut [u8]) -> std::result::Result<usize, i32> {
    call(125, &ticket.to_le_bytes(), rgba)
}
pub fn nv2a_validate(block: &[u8], cube_mask: u32) -> std::result::Result<(), i32> {
    if block.len() != 13_072 || cube_mask > 15 {
        return Err(-1);
    }
    let mut request = cube_mask.to_le_bytes().to_vec();
    request.extend(block);
    call(126, &request, &mut []).map(|_| ())
}
/// Canonical 27-word generic render state; see PORT_SERVICES.md. Template
/// index names verified host-prepared shaders, never game shader source.
pub fn pipeline_prepare(template: u32, state: &[u32; 27]) -> std::result::Result<u64, i32> {
    let request = [template]
        .into_iter()
        .chain(*state)
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut reply = [0; 8];
    if call(127, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn pipeline_poll(variant: u64) -> std::result::Result<[u8; 32], i32> {
    let mut hash = [0; 32];
    if call(128, &variant.to_le_bytes(), &mut hash)? != 32 {
        return Err(-3);
    }
    Ok(hash)
}
pub fn target_pass_state(
    target: u64,
    mask: u32,
    color: u32,
    depth: f32,
    draws: &[IndexedDraw<'_>],
    states: &[DrawState<'_>],
    query: Option<u64>,
) -> std::result::Result<(), i32> {
    if draws.len() != states.len() || draws.len() > 4096 {
        return Err(-1);
    }
    let mut request = [target.to_le_bytes(), query.unwrap_or(0).to_le_bytes()].concat();
    for n in [mask, color, depth.to_bits(), draws.len() as u32] {
        request.extend(n.to_le_bytes());
    }
    for (draw, state) in draws.iter().zip(states) {
        if !state.samplers.is_empty() && state.samplers.len() != draw.textures.len() {
            return Err(-1);
        }
        let record = indexed_bytes(0, 0, 0, 0., std::slice::from_ref(draw))?;
        if state.pipeline_variant == 0 {
            request.extend(&record[20..24]);
        } else {
            request.extend(u32::MAX.to_le_bytes());
            request.extend(state.pipeline_variant.to_le_bytes());
        }
        for f in state.blend_constant {
            request.extend(f.to_le_bytes());
        }
        request.extend(state.stencil_reference.to_le_bytes());
        request.extend(&record[24..80]);
        for (slot, binding) in draw.textures.iter().enumerate() {
            request.extend(binding.texture.to_le_bytes());
            let sampler = state.samplers.get(slot).copied().unwrap_or(0);
            request.extend(
                if sampler == 0 {
                    binding.sampler
                } else {
                    0x8000_0000u32
                }
                .to_le_bytes(),
            );
            if sampler != 0 {
                request.extend(sampler.to_le_bytes());
            }
        }
    }
    call(123, &request, &mut []).map(|_| ())
}
pub fn draw_indexed(
    width: u32,
    height: u32,
    clear: u32,
    depth: f32,
    draws: &[IndexedDraw<'_>],
) -> std::result::Result<(), i32> {
    let request = indexed_bytes(width, height, clear, depth, draws)?;
    call(105, &request, &mut [])?;
    Ok(())
}
/// Ordered retained pass. Mask bits 1/2 clear color/depth; zero loads both.
/// Optional query brackets the complete pass. Pipelines remain host-installed.
pub fn target_pass(
    target: u64,
    clear_mask: u32,
    color: u32,
    depth: f32,
    draws: &[IndexedDraw<'_>],
    query: Option<u64>,
) -> std::result::Result<(), i32> {
    let encoded = indexed_bytes(0, 0, color, depth, draws)?;
    let mut request = target.to_le_bytes().to_vec();
    if let Some(id) = query {
        request.extend(id.to_le_bytes());
    }
    request.extend(clear_mask.to_le_bytes());
    request.extend(&encoded[8..]);
    call(if query.is_some() { 115 } else { 108 }, &request, &mut []).map(|_| ())
}
fn indexed_bytes(
    width: u32,
    height: u32,
    clear: u32,
    depth: f32,
    draws: &[IndexedDraw<'_>],
) -> std::result::Result<Vec<u8>, i32> {
    if draws.len() > 4096 || draws.iter().any(|d| d.textures.len() > 8) {
        return Err(-1);
    }
    let mut request = [width, height, clear, depth.to_bits(), draws.len() as u32]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    for d in draws {
        request.extend(d.pipeline.to_le_bytes());
        for id in [d.vertex, d.index, d.uniform] {
            request.extend(id.to_le_bytes());
        }
        for n in [
            d.first_index,
            d.index_count,
            d.base_vertex as u32,
            d.scissor[0],
            d.scissor[1],
            d.scissor[2],
            d.scissor[3],
            d.textures.len() as u32,
        ] {
            request.extend(n.to_le_bytes());
        }
        for binding in d.textures {
            request.extend(binding.texture.to_le_bytes());
            request.extend(binding.sampler.to_le_bytes());
        }
    }
    Ok(request)
}

/// Clear a caller-owned byte slice with non-elidable stores. No transport,
/// allocation, pointer exposure, lock or native system call is involved.
/// Oversized slices are rejected before any write. This covers this slice,
/// not other copies, registers, swap, crash dumps or historical allocations.
pub fn zeroize(bytes: &mut [u8]) -> std::result::Result<(), i32> {
    if bytes.len() > 1024 * 1024 {
        return Err(-1);
    }
    for byte in bytes {
        // SAFETY: each byte is exclusively borrowed and valid for one write.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    Ok(())
}

// Local services execute in the confined CHILD, not the remote window owner.
// Bootstrap installs fixed windows and a reviewed task table before game code.
unsafe extern "C" {
    #[link_name = "gamepkg_local_call"]
    fn local_trampoline(
        operation: u32,
        request: *const u8,
        length: u32,
        reply: *mut u8,
        capacity: u32,
    ) -> i32;
}
pub fn local_call(
    operation: u32,
    request: &[u8],
    reply: &mut [u8],
) -> std::result::Result<usize, i32> {
    if request.len() > 1024 * 1024 || reply.len() > 1024 * 1024 {
        return Err(-1);
    }
    let result = unsafe {
        local_trampoline(
            operation,
            request.as_ptr(),
            request.len() as u32,
            reply.as_mut_ptr(),
            reply.len() as u32,
        )
    };
    if result < 0 {
        Err(result)
    } else if result as usize > reply.len() {
        Err(-3)
    } else {
        Ok(result as usize)
    }
}
/// Create a session-local opaque record, 1..4096 bytes. At most 64 records
/// and 64 KiB payload are retained by the trusted local runtime.
pub fn local_record_create(bytes: &[u8]) -> std::result::Result<u64, i32> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err(-1);
    }
    let mut reply = [0u8; 8];
    if local_call(34, bytes, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn local_record_snapshot(
    record: u64,
    maximum: usize,
) -> std::result::Result<(u64, Vec<u8>), i32> {
    if maximum == 0 || maximum > 4096 {
        return Err(-1);
    }
    let mut reply = vec![0u8; maximum + 8];
    let length = local_call(35, &record.to_le_bytes(), &mut reply)?;
    if !(9..=maximum + 8).contains(&length) {
        return Err(-3);
    }
    Ok((
        u64::from_le_bytes(fixed8(&reply, 0)),
        reply[8..length].to_vec(),
    ))
}
/// Busy is a stale snapshot, without any write. Overflow fails without wrap.
pub fn local_record_compare_exchange(
    record: u64,
    version: u64,
    bytes: &[u8],
) -> std::result::Result<u64, i32> {
    if bytes.is_empty() || bytes.len() > 4096 {
        return Err(-1);
    }
    let mut request = record.to_le_bytes().to_vec();
    request.extend(version.to_le_bytes());
    request.extend(bytes);
    let mut reply = [0u8; 8];
    if local_call(36, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
/// Revoke this record ID and every copied reference to it; IDs never reuse.
pub fn local_record_release(record: u64) -> std::result::Result<(), i32> {
    if local_call(37, &record.to_le_bytes(), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
/// Copy at most 64 MiB between live child-local RAM windows. Both complete
/// ranges/permissions are validated before writing; overlap is safe. No pointer.
pub fn local_memory_copy(
    source: u64,
    source_offset: u64,
    target: u64,
    target_offset: u64,
    bytes: u64,
) -> std::result::Result<(), i32> {
    if bytes > 64 * 1024 * 1024 {
        return Err(-1);
    }
    let request = [
        source.to_le_bytes(),
        source_offset.to_le_bytes(),
        target.to_le_bytes(),
        target_offset.to_le_bytes(),
        bytes.to_le_bytes(),
    ]
    .concat();
    if local_call(7, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
/// Compare and replace 1..65536 bytes atomically in a live child-local window.
/// Both slices must have the same length. True means replaced; false means a
/// stale comparison with no write. Bounds and all write permissions are checked
/// before comparison; no pointer or mapped slice is exposed. Larger copied
/// game metadata belongs in explicitly budgeted local memory, not opaque records.
pub fn local_memory_compare_exchange(
    window: u64,
    offset: u64,
    expected: &[u8],
    replacement: &[u8],
) -> std::result::Result<bool, i32> {
    if expected.is_empty() || expected.len() > 65536 || expected.len() != replacement.len() {
        return Err(-1);
    }
    let mut request = window.to_le_bytes().to_vec();
    request.extend(offset.to_le_bytes());
    request.extend((expected.len() as u32).to_le_bytes());
    request.extend(expected);
    request.extend(replacement);
    let mut output = [0u8; 4];
    if local_call(42, &request, &mut output)? != 4 {
        return Err(-3);
    }
    match u32::from_le_bytes(output) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(-3),
    }
}
/// Fill at most 64 MiB in a live child-local RAM window. The whole destination
/// must be committed and writable before any byte changes. No native pointer.
pub fn local_memory_fill(
    window: u64,
    offset: u64,
    bytes: u64,
    value: u8,
) -> std::result::Result<(), i32> {
    if bytes > 64 * 1024 * 1024 {
        return Err(-1);
    }
    let request = [
        window.to_le_bytes(),
        offset.to_le_bytes(),
        bytes.to_le_bytes(),
        u64::from(value).to_le_bytes(),
    ]
    .concat();
    if local_call(8, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
// Encode bounded opaque descriptors. This is trusted generated binding code,
// injected only after the complete dependency-free game source was checked.
fn memory_segments_request(segments: &[[u64; 3]]) -> std::result::Result<(Vec<u8>, usize), i32> {
    if segments.is_empty() || segments.len() > 32 {
        return Err(-1);
    }
    let mut request = (segments.len() as u32).to_le_bytes().to_vec();
    let mut total = 0u64;
    for segment in segments {
        if segment[2] == 0 {
            return Err(-1);
        }
        total = total.checked_add(segment[2]).ok_or(-1)?;
        if total > 1024 * 1024 {
            return Err(-1);
        }
        for value in segment {
            request.extend(value.to_le_bytes());
        }
    }
    Ok((request, total as usize))
}
/// Concatenate 1..32 opaque RAM ranges into an exact-size output (<=1 MiB).
/// All descriptors/permissions are checked before any read under one lock.
pub fn local_memory_read_segments(
    segments: &[[u64; 3]],
    output: &mut [u8],
) -> std::result::Result<(), i32> {
    let (request, total) = memory_segments_request(segments)?;
    if output.len() != total {
        return Err(-1);
    }
    if local_call(9, &request, output)? != total {
        return Err(-3);
    }
    Ok(())
}
/// Scatter one copied payload in descriptor order; later aliases win. All
/// ranges must be writable before any change. Header plus payload <=1 MiB.
pub fn local_memory_write_segments(
    segments: &[[u64; 3]],
    input: &[u8],
) -> std::result::Result<(), i32> {
    let (mut request, total) = memory_segments_request(segments)?;
    if input.len() != total || request.len() + total > 1024 * 1024 {
        return Err(-1);
    }
    request.extend(input);
    if local_call(15, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
/// Copy 1..256 page states from an opaque child-local RAM window. Offset
/// must be 4 KiB aligned. Values: 0 uncommitted, 1 no-access, 2 read-only,
/// 3 read/write. This changes no memory/permission and grants no host address.
pub fn local_memory_page_states(
    window: u64,
    offset: u64,
    pages: u32,
) -> std::result::Result<Vec<u8>, i32> {
    if offset % 4096 != 0 || !(1..=256).contains(&pages) {
        return Err(-1);
    }
    let request = [
        window.to_le_bytes().as_slice(),
        &offset.to_le_bytes(),
        &pages.to_le_bytes(),
        &0u32.to_le_bytes(),
    ]
    .concat();
    let mut output = vec![0u8; pages as usize];
    if local_call(17, &request, &mut output)? != output.len()
        || output.iter().any(|state| *state > 3)
    {
        return Err(-3);
    }
    Ok(output)
}
/// Acquire only an actual child-local recursive mutex (0..1000 ms). Events
/// reject without consuming their signal. Cancelled reviewed tasks are denied.
pub fn local_mutex_lock(mutex: u64, milliseconds: u32) -> std::result::Result<(), i32> {
    if milliseconds > 1000 {
        return Err(-1);
    }
    let request = [mutex.to_le_bytes().as_slice(), &milliseconds.to_le_bytes()].concat();
    if local_call(26, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
/// Wait only on an actual child-local event (0..1000 ms). A mutex is rejected
/// without acquiring it. Auto-reset signals are consumed only on success.
pub fn local_event_wait(event: u64, milliseconds: u32) -> std::result::Result<(), i32> {
    if milliseconds > 1000 {
        return Err(-1);
    }
    let request = [event.to_le_bytes().as_slice(), &milliseconds.to_le_bytes()].concat();
    if local_call(27, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalEventState {
    pub manual: bool,
    pub signaled: bool,
}
/// Snapshot event kind/state without consuming or resetting a signal. This
/// value is not a lease; subsequent use rechecks revocation and cancellation.
pub fn local_event_state(event: u64) -> std::result::Result<LocalEventState, i32> {
    let mut output = [0u8; 8];
    if local_call(28, &event.to_le_bytes(), &mut output)? != 8 {
        return Err(-3);
    }
    let manual = u32::from_le_bytes(fixed4(&output, 0));
    let signaled = u32::from_le_bytes(fixed4(&output, 4));
    if manual > 1 || signaled > 1 {
        return Err(-3);
    }
    Ok(LocalEventState {
        manual: manual != 0,
        signaled: signaled != 0,
    })
}
/// Create a child-local counted semaphore. Maximum is 1..=i32::MAX, and initial
/// must not exceed it. Uses the shared 128 synchronization-object budget.
pub fn local_semaphore_create(initial: u32, maximum: u32) -> std::result::Result<u64, i32> {
    if maximum == 0 || maximum > 0x7fffffff || initial > maximum {
        return Err(-1);
    }
    let mut output = [0u8; 8];
    if local_call(
        43,
        &[initial.to_le_bytes(), maximum.to_le_bytes()].concat(),
        &mut output,
    )? != 8
    {
        return Err(-3);
    }
    let id = u64::from_le_bytes(output);
    if id == 0 {
        return Err(-3);
    }
    Ok(id)
}
/// Atomically release 1..=i32::MAX permits and return the previous count.
/// Overflow/exceeding the maximum fails without a partial release.
pub fn local_semaphore_release(semaphore: u64, permits: u32) -> std::result::Result<u32, i32> {
    if permits == 0 || permits > 0x7fffffff {
        return Err(-1);
    }
    let mut output = [0u8; 4];
    let request = [semaphore.to_le_bytes().as_slice(), &permits.to_le_bytes()].concat();
    if local_call(44, &request, &mut output)? != 4 {
        return Err(-3);
    }
    let previous = u32::from_le_bytes(output);
    if previous > 0x7fffffff {
        return Err(-3);
    }
    Ok(previous)
}
/// Acquire one permit, with a cancellable 0..1000 ms wait. Other synchronization
/// kinds reject without changing their state. Busy means no permit at deadline.
pub fn local_semaphore_wait(semaphore: u64, milliseconds: u32) -> std::result::Result<(), i32> {
    if milliseconds > 1000 {
        return Err(-1);
    }
    let request = [
        semaphore.to_le_bytes().as_slice(),
        &milliseconds.to_le_bytes(),
    ]
    .concat();
    if local_call(45, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
/// Retire only an actual event (1), mutex (2), or semaphore (3). Unlike an
/// ownership unlock, this refuses a held mutex. Available for cancelled-task
/// cleanup while the runtime is open; copied observations grant no lifetime.
pub fn local_sync_release(id: u64, kind: u32) -> std::result::Result<(), i32> {
    if !(1..=3).contains(&kind) {
        return Err(-1);
    }
    let request = [id.to_le_bytes().as_slice(), &kind.to_le_bytes()].concat();
    if local_call(49, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
/// Non-consuming copied mutex state, not an ownership token or lifetime lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalMutexState {
    pub locked: bool,
    pub owned_by_caller: bool,
    pub abandoned: bool,
    pub depth: u32,
}
pub fn local_mutex_state(mutex: u64) -> std::result::Result<LocalMutexState, i32> {
    let mut output = [0u8; 16];
    if local_call(48, &mutex.to_le_bytes(), &mut output)? != 16 {
        return Err(-3);
    }
    let locked = u32::from_le_bytes(fixed4(&output, 0));
    let owned = u32::from_le_bytes(fixed4(&output, 4));
    let abandoned = u32::from_le_bytes(fixed4(&output, 8));
    let depth = u32::from_le_bytes(fixed4(&output, 12));
    if locked > 1
        || owned > 1
        || abandoned > 1
        || (owned != 0 && locked == 0)
        || (locked != 0) != (depth != 0)
        || (abandoned != 0 && locked != 0)
    {
        return Err(-3);
    }
    Ok(LocalMutexState {
        locked: locked != 0,
        owned_by_caller: owned != 0,
        abandoned: abandoned != 0,
        depth,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalSemaphoreState {
    pub count: u32,
    pub maximum: u32,
}
/// Non-consuming copied state, not a lease. Release with existing local op 25.
pub fn local_semaphore_state(semaphore: u64) -> std::result::Result<LocalSemaphoreState, i32> {
    let mut output = [0u8; 8];
    if local_call(46, &semaphore.to_le_bytes(), &mut output)? != 8 {
        return Err(-3);
    }
    let count = u32::from_le_bytes(fixed4(&output, 0));
    let maximum = u32::from_le_bytes(fixed4(&output, 4));
    if maximum == 0 || maximum > 0x7fffffff || count > maximum {
        return Err(-3);
    }
    Ok(LocalSemaphoreState { count, maximum })
}
/// Typed opaque child-local object. Kinds: event=1, mutex=2, semaphore=3,
/// reviewed task completion=4. The runtime verifies the actual kind/lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalWaitObject {
    pub id: u64,
    pub kind: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalWaitResult {
    pub index: u32,
    pub abandoned: bool,
}
/// Atomically acquire all, or the first available in caller order. Task entries
/// are non-consuming completion observations, not joins. Busy is a deadline.
/// Abandoned mutexes are acquired and explicitly reported, never normal success.
pub fn local_wait_many(
    objects: &[LocalWaitObject],
    all: bool,
    milliseconds: u32,
) -> std::result::Result<LocalWaitResult, i32> {
    if objects.is_empty() || objects.len() > 64 || milliseconds > 1000 {
        return Err(-1);
    }
    let mut request = Vec::with_capacity(12 + objects.len() * 12);
    request.extend((objects.len() as u32).to_le_bytes());
    request.extend(u32::from(all).to_le_bytes());
    request.extend(milliseconds.to_le_bytes());
    for (index, object) in objects.iter().enumerate() {
        if object.id == 0
            || !(1..=4).contains(&object.kind)
            || objects[..index].iter().any(|old| old.id == object.id)
        {
            return Err(-1);
        }
        request.extend(object.id.to_le_bytes());
        request.extend(object.kind.to_le_bytes());
    }
    let mut output = [0u8; 8];
    if local_call(47, &request, &mut output)? != 8 {
        return Err(-3);
    }
    let index = u32::from_le_bytes(fixed4(&output, 0));
    let reason = u32::from_le_bytes(fixed4(&output, 4));
    if index as usize >= objects.len()
        || reason > 1
        || all && reason == 0 && index != 0
        || reason == 1 && objects[index as usize].kind != 2
    {
        return Err(-3);
    }
    Ok(LocalWaitResult {
        index,
        abandoned: reason == 1,
    })
}
/// Current reviewed task ID and cooperative cancellation flag. ID zero is the
/// bootstrap thread. Revocation returns Denied rather than a stale status.
pub fn local_task_status() -> std::result::Result<(u64, bool), i32> {
    let mut reply = [0; 12];
    if local_call(14, &[], &mut reply)? != 12 {
        return Err(-3);
    }
    let cancelled = u32::from_le_bytes(fixed4(&reply, 8));
    if cancelled > 1 {
        return Err(-3);
    }
    Ok((u64::from_le_bytes(fixed8(&reply, 0)), cancelled != 0))
}
/// Evidence of one successful consuming local join. The private fields can
/// only be constructed by this host-generated binding, not checked game code.
/// It owns no native object and does not keep a job alive or make it joinable.
pub struct LocalTaskJoin {
    task: u64,
    value: u64,
}
impl LocalTaskJoin {
    pub fn task_id(&self) -> u64 {
        self.task
    }
    pub fn result(&self) -> u64 {
        self.value
    }
}
/// Nonblocking actual join (local operation 11). Busy preserves the live job;
/// success consumes it exactly once and returns evidence for shared owners.
pub fn local_task_join(task: u64) -> std::result::Result<LocalTaskJoin, i32> {
    let mut output = [0u8; 8];
    if local_call(11, &task.to_le_bytes(), &mut output)? != 8 {
        return Err(-3);
    }
    Ok(LocalTaskJoin {
        task,
        value: u64::from_le_bytes(output),
    })
}
/// Non-consuming completion snapshot. A finished task remains joinable; a
/// missing/revoked task is Denied. This never resumes a suspended task.
pub fn local_task_finished(task: u64) -> std::result::Result<bool, i32> {
    let mut reply = [0u8; 4];
    if local_call(16, &task.to_le_bytes(), &mut reply)? != 4 {
        return Err(-3);
    }
    match u32::from_le_bytes(reply) {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(-3),
    }
}
/// Child-local bounded copied-message queue. No raw threads or OS endpoints.
pub fn local_mailbox_create(maximum: u32, depth: u32) -> std::result::Result<u64, i32> {
    let request = [maximum.to_le_bytes(), depth.to_le_bytes()].concat();
    let mut reply = [0; 8];
    if local_call(30, &request, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn local_mailbox_send(id: u64, message: &[u8]) -> std::result::Result<(), i32> {
    if message.len() > 65536 {
        return Err(-1);
    }
    let mut request = id.to_le_bytes().to_vec();
    request.extend(message);
    local_call(31, &request, &mut []).map(|_| ())
}
/// Timeout is at most one second. Busy means empty at timeout; Invalid for a
/// short reply buffer preserves the queued message. Empty messages are valid.
pub fn local_mailbox_receive(
    id: u64,
    milliseconds: u32,
    reply: &mut [u8],
) -> std::result::Result<usize, i32> {
    let mut request = id.to_le_bytes().to_vec();
    request.extend(milliseconds.to_le_bytes());
    local_call(32, &request, reply)
}
pub fn local_mailbox_release(id: u64) -> std::result::Result<(), i32> {
    local_call(33, &id.to_le_bytes(), &mut []).map(|_| ())
}
pub fn audio_stream_create(layout: u32, frames: u32) -> std::result::Result<u64, i32> {
    let bytes = [0u32, 48000, layout, frames]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut reply = [0; 8];
    if call(130, &bytes, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn audio_stream_submit(id: u64, pcm: &[i16]) -> std::result::Result<(), i32> {
    if pcm.len() > 1600 * 6 {
        return Err(-1);
    }
    let mut request = id.to_le_bytes().to_vec();
    request.extend(pcm.iter().flat_map(|s| s.to_le_bytes()));
    call(131, &request, &mut [])?;
    Ok(())
}
pub fn audio_stream_status(id: u64) -> std::result::Result<[u64; 4], i32> {
    let mut reply = [0; 40];
    if call(132, &id.to_le_bytes(), &mut reply)? != 40 {
        return Err(-3);
    }
    Ok(std::array::from_fn(|i| {
        u64::from_le_bytes(fixed8(&reply, i * 8))
    }))
}
pub fn audio_stream_control(id: u64, action: u32) -> std::result::Result<(), i32> {
    let mut bytes = id.to_le_bytes().to_vec();
    bytes.extend(action.to_le_bytes());
    call(133, &bytes, &mut [])?;
    Ok(())
}
pub fn audio_stream_release(id: u64) -> std::result::Result<(), i32> {
    call(134, &id.to_le_bytes(), &mut [])?;
    Ok(())
}
pub fn capture_create(layout: u32, frames: u32) -> std::result::Result<u64, i32> {
    let bytes = [0u32, 48000, layout, frames]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let mut reply = [0; 8];
    if call(140, &bytes, &mut reply)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(reply))
}
pub fn capture_read(id: u64, frames: u32, reply: &mut [u8]) -> std::result::Result<usize, i32> {
    let mut bytes = id.to_le_bytes().to_vec();
    bytes.extend(frames.to_le_bytes());
    call(141, &bytes, reply)
}
pub fn capture_status(id: u64) -> std::result::Result<[u64; 4], i32> {
    let mut reply = [0; 32];
    if call(142, &id.to_le_bytes(), &mut reply)? != 32 {
        return Err(-3);
    }
    Ok(std::array::from_fn(|i| {
        u64::from_le_bytes(fixed8(&reply, i * 8))
    }))
}
pub fn capture_release(id: u64) -> std::result::Result<(), i32> {
    call(143, &id.to_le_bytes(), &mut [])?;
    Ok(())
}

#[cfg(all(test, unix))]
pub(crate) static TEST_BINDING_LOCK: Mutex<()> = Mutex::new(());

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::{fd::OwnedFd, unix::net::UnixStream};
    fn file(s: UnixStream) -> File {
        File::from(OwnedFd::from(s))
    }
    #[test]
    fn performance_samples_validate_exact_shape_and_unaligned_units() {
        let expected = PerformanceSample {
            counter: 0x123456789abcdef,
            frequency: 3_579_545,
            uptime_ms: u64::MAX,
        };
        let mut bytes = vec![0xee];
        for value in [expected.counter, expected.frequency, expected.uptime_ms] {
            bytes.extend(value.to_le_bytes());
        }
        assert_eq!(performance_sample_decode(&bytes[1..]), Ok(expected));
        for length in 0..24 {
            assert_eq!(performance_sample_decode(&bytes[1..1 + length]), Err(-3));
        }
        assert_eq!(performance_sample_decode(&[0u8; 25]), Err(-3));
        for (counter, frequency) in [(1u64, 0u64), (u64::MAX, 1), (1, u64::MAX)] {
            let mut bytes = [0u8; 24];
            bytes[..8].copy_from_slice(&counter.to_le_bytes());
            bytes[8..16].copy_from_slice(&frequency.to_le_bytes());
            assert_eq!(performance_sample_decode(&bytes), Err(-3));
        }
    }
    #[test]
    fn clock_samples_decode_unaligned_fixed_width_fields_and_reject_malformed_values() {
        let expected = ClockSample {
            counter: 0x123456789abcdef,
            frequency: 1_000_000_000,
            uptime_ms: 0x23456789abcdef01,
            utc_100ns: 0x3456789abcdef012,
        };
        let mut data = vec![0xa5];
        for value in [
            expected.counter,
            expected.frequency,
            expected.uptime_ms,
            expected.utc_100ns,
        ] {
            data.extend(value.to_le_bytes());
        }
        assert_eq!(clock_sample_decode(&data[1..]), Ok(expected));
        for length in 0..32 {
            assert_eq!(clock_sample_decode(&data[1..1 + length]), Err(-3));
        }
        assert_eq!(clock_sample_decode(&[0u8; 33]), Err(-3));
        for (counter, frequency) in [(1u64, 0u64), (u64::MAX, 1), (1, u64::MAX)] {
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&counter.to_le_bytes());
            bytes[8..16].copy_from_slice(&frequency.to_le_bytes());
            assert_eq!(clock_sample_decode(&bytes), Err(-3));
        }
    }
    #[test]
    fn fixed_decoders_preserve_unaligned_little_endian_bits_and_trap_short_buffers() {
        let wire = [0xee, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xdd];
        assert_eq!(u16::from_le_bytes(fixed2(&wire, 1)), 0x2301);
        assert_eq!(u32_at(&wire, 1), 0x67452301);
        assert_eq!(u64_at(&wire, 1), 0xefcdab8967452301);
        for length in 0..8 {
            assert!(std::panic::catch_unwind(|| fixed8(&wire[..length], 0)).is_err());
        }
        for length in 0..4 {
            assert!(std::panic::catch_unwind(|| fixed4(&wire[..length], 0)).is_err());
        }
        for length in 0..2 {
            assert!(std::panic::catch_unwind(|| fixed2(&wire[..length], 0)).is_err());
        }
    }
    #[test]
    fn local_text_validation_is_bounded_and_matches_standard_utf8_for_binary_inputs() {
        for value in 0..=u16::MAX {
            let bytes = value.to_le_bytes();
            assert_eq!(
                text_from_utf8(&bytes).is_ok(),
                std::str::from_utf8(&bytes).is_ok()
            );
        }
        for bytes in [
            b"".as_slice(),
            b"plain",
            "café 日本語 🎮".as_bytes(),
            &[0xf4, 0x8f, 0xbf, 0xbf],
            &[0xf4, 0x90, 0x80, 0x80],
            &[0xed, 0xa0, 0x80],
            &[0xc0, 0x80],
            &[0xe2, 0x82],
            &[0xff],
        ] {
            assert_eq!(text_from_utf8(bytes).ok(), std::str::from_utf8(bytes).ok());
        }
        assert_eq!(
            unsafe { gamepkg_port_utf8_validate(std::ptr::null(), 0, 0) },
            0
        );
        assert_eq!(
            unsafe { gamepkg_port_utf8_validate(std::ptr::null(), 1, 0) },
            -1
        );
        assert_eq!(
            unsafe { gamepkg_port_utf8_validate(std::ptr::null(), MAX as u32 + 1, 0) },
            -1
        );
        assert_eq!(text_from_utf8(&vec![0; MAX + 1]), Err(-1));
        assert_eq!(
            unsafe { gamepkg_port_utf8_validate(std::ptr::null(), 0, 2) },
            -1
        );
        for name in [
            "",
            ".",
            "..",
            "a/b",
            "a\\b",
            "a\0",
            "line\n",
            "control\u{85}",
        ] {
            assert_eq!(text_component(name.as_bytes()), Err(-1));
        }
        assert_eq!(text_component("日本語".as_bytes()), Ok("日本語"));
    }
    #[test]
    fn malformed_directory_entries_fail_before_they_become_game_paths() {
        let wire = |kind: u32, name: &[u8]| {
            [
                kind.to_le_bytes().as_slice(),
                &(name.len() as u32).to_le_bytes(),
                name,
            ]
            .concat()
        };
        let mut valid = wire(1, "café".as_bytes());
        valid.extend(wire(2, b"folder"));
        let entries = decode_directory_entries(&valid).unwrap();
        assert_eq!(entries[0].name, "café");
        assert!(!entries[0].directory);
        assert!(entries[1].directory);
        assert!(decode_directory_entries(&[]).unwrap().is_empty());
        for invalid in [
            vec![0; 7],
            wire(0, b"file"),
            wire(3, b"file"),
            wire(1, b""),
            wire(1, &[b'a'; 256]),
            wire(1, b"a/b"),
            wire(1, b".."),
            wire(1, &[0xff]),
            wire(1, b"a\0"),
            wire(1, "\u{85}".as_bytes()),
            [1u32.to_le_bytes(), 5u32.to_le_bytes()].concat(),
            wire(1, b"x").repeat(4097),
        ] {
            assert!(decode_directory_entries(&invalid).is_err());
        }
    }
    #[test]
    fn malformed_host_replies_are_rejected_before_copying_or_allocating_payloads() {
        for (status, len) in [(0i32, MAX as u32 + 1), (1, 0), (-9, 0), (-4, 1)] {
            let (local, mut remote) = UnixStream::pair().unwrap();
            let worker = std::thread::spawn(move || {
                let mut header = [0; 16];
                remote.read_exact(&mut header).unwrap();
                remote.write_all(&status.to_le_bytes()).unwrap();
                remote.write_all(&len.to_le_bytes()).unwrap();
            });
            let mut client = Pipe {
                input: file(local.try_clone().unwrap()),
                output: file(local),
            };
            assert!(client.call(0, &[], 4).is_err());
            worker.join().unwrap();
        }
    }
    #[test]
    fn concurrent_callers_are_serialized_and_broken_transport_stays_revoked() {
        let _exclusive = TEST_BINDING_LOCK.lock().unwrap();
        let (local, mut remote) = UnixStream::pair().unwrap();
        connect(file(local.try_clone().unwrap()), file(local)).unwrap();
        let worker = std::thread::spawn(move || {
            for _ in 0..32 {
                let mut header = [0; 16];
                remote.read_exact(&mut header).unwrap();
                assert_eq!(&header[..4], b"GPH1");
                let len = u32::from_le_bytes(header[8..12].try_into().unwrap());
                assert_eq!(len, 4);
                let mut payload = [0; 4];
                remote.read_exact(&mut payload).unwrap();
                remote.write_all(&0i32.to_le_bytes()).unwrap();
                remote.write_all(&4u32.to_le_bytes()).unwrap();
                remote.write_all(&payload).unwrap();
            }
            let mut header = [0; 16];
            remote.read_exact(&mut header).unwrap();
            remote.write_all(&0i32.to_le_bytes()).unwrap();
            remote.write_all(&u32::MAX.to_le_bytes()).unwrap();
        });
        let callers: Vec<_> = (0..32u32)
            .map(|n| {
                std::thread::spawn(move || {
                    let mut out = [0; 4];
                    assert_eq!(call(99, &n.to_le_bytes(), &mut out), Ok(4));
                    assert_eq!(out, n.to_le_bytes());
                })
            })
            .collect();
        for c in callers {
            c.join().unwrap();
        }
        assert_eq!(call(0, &[], &mut [0; 4]), Err(-3));
        assert_eq!(call(0, &[], &mut [0; 4]), Err(-4));
        worker.join().unwrap();
        disconnect();
        // Verify the advanced binding against the documented wire offsets,
        // including an owned sampler after the fixed 80-byte draw record.
        let (local, mut remote) = UnixStream::pair().unwrap();
        connect(file(local.try_clone().unwrap()), file(local)).unwrap();
        let worker = std::thread::spawn(move || {
            let mut header = [0; 16];
            remote.read_exact(&mut header).unwrap();
            assert_eq!(u32::from_le_bytes(header[4..8].try_into().unwrap()), 123);
            assert_eq!(u32::from_le_bytes(header[8..12].try_into().unwrap()), 132);
            let mut bytes = [0; 132];
            remote.read_exact(&mut bytes).unwrap();
            assert_eq!(u64::from_le_bytes(bytes[..8].try_into().unwrap()), 7);
            assert_eq!(u64::from_le_bytes(bytes[8..16].try_into().unwrap()), 9);
            assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 1);
            assert_eq!(u32::from_le_bytes(bytes[32..36].try_into().unwrap()), 3);
            assert_eq!(f32::from_le_bytes(bytes[36..40].try_into().unwrap()), 0.25);
            assert_eq!(u32::from_le_bytes(bytes[52..56].try_into().unwrap()), 17);
            assert_eq!(u64::from_le_bytes(bytes[56..64].try_into().unwrap()), 4);
            assert_eq!(u32::from_le_bytes(bytes[108..112].try_into().unwrap()), 1);
            assert_eq!(u64::from_le_bytes(bytes[112..120].try_into().unwrap()), 11);
            assert_eq!(
                u32::from_le_bytes(bytes[120..124].try_into().unwrap()),
                0x80000000
            );
            assert_eq!(u64::from_le_bytes(bytes[124..132].try_into().unwrap()), 13);
            remote.write_all(&[0; 8]).unwrap();
            remote.read_exact(&mut header).unwrap();
            assert_eq!(u32::from_le_bytes(header[8..12].try_into().unwrap()), 140);
            let mut variant = [0; 140];
            remote.read_exact(&mut variant).unwrap();
            assert_eq!(
                u32::from_le_bytes(variant[32..36].try_into().unwrap()),
                u32::MAX
            );
            assert_eq!(u64::from_le_bytes(variant[36..44].try_into().unwrap()), 29);
            assert_eq!(
                f32::from_le_bytes(variant[44..48].try_into().unwrap()),
                0.25
            );
            assert_eq!(u32::from_le_bytes(variant[60..64].try_into().unwrap()), 17);
            assert_eq!(
                u64::from_le_bytes(variant[132..140].try_into().unwrap()),
                13
            );
            remote.write_all(&[0; 8]).unwrap();
        });
        let textures = [TextureBinding {
            texture: 11,
            sampler: 0,
        }];
        let draw = IndexedDraw {
            pipeline: 3,
            vertex: 4,
            index: 5,
            uniform: 6,
            first_index: 0,
            index_count: 3,
            base_vertex: 0,
            scissor: [0, 0, 16, 16],
            textures: &textures,
        };
        let samplers = [13];
        let state = DrawState {
            pipeline_variant: 0,
            blend_constant: [0.25, 0.5, 0.75, 1.],
            stencil_reference: 17,
            samplers: &samplers,
        };
        target_pass_state(7, 3, 0, 1., &[draw], &[state], Some(9)).unwrap();
        target_pass_state(
            7,
            3,
            0,
            1.,
            &[draw],
            &[DrawState {
                pipeline_variant: 29,
                ..state
            }],
            Some(9),
        )
        .unwrap();
        worker.join().unwrap();
        disconnect();
    }
    #[test]
    fn committed_durability_error_is_a_valid_reply_not_a_transport_failure() {
        let (local, mut remote) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            let mut header = [0; 16];
            remote.read_exact(&mut header).unwrap();
            remote.write_all(&(-6i32).to_le_bytes()).unwrap();
            remote.write_all(&0u32.to_le_bytes()).unwrap();
        });
        let mut client = Pipe {
            input: file(local.try_clone().unwrap()),
            output: file(local),
        };
        assert_eq!(client.call(14, &[], 0).unwrap(), Err(-6));
        worker.join().unwrap();
    }
}

#[cfg(test)]
mod local_clock_decode_tests {
    use super::*;
    fn state(counter: u64, frequency: u64, generation: u64) -> Vec<u8> {
        [
            counter.to_le_bytes(),
            frequency.to_le_bytes(),
            123u64.to_le_bytes(),
            generation.to_le_bytes(),
        ]
        .concat()
    }
    #[test]
    fn local_clock_state_is_exact_unaligned_and_fixed_width() {
        let value = state(500, 1000, 7);
        let mut padded = vec![0xff];
        padded.extend(&value);
        let decoded = local_clock_state_decode(&padded[1..]).unwrap();
        assert_eq!(decoded.sample.counter, 500);
        assert_eq!(decoded.sample.frequency, 1000);
        assert_eq!(decoded.sample.uptime_ms, 123);
        assert_eq!(decoded.generation, 7);
        for length in 0..32 {
            assert_eq!(local_clock_state_decode(&value[..length]), Err(-3));
        }
        assert_eq!(
            local_clock_state_decode(&[value.clone(), vec![0]].concat()),
            Err(-3)
        );
        for value in [
            state(u64::MAX, 1000, 0),
            state(0, 0, 0),
            state(0, u64::MAX, 0),
        ] {
            assert_eq!(local_clock_state_decode(&value), Err(-3));
        }
    }
    #[test]
    fn local_clock_wait_replies_cannot_fabricate_expiry_or_wakeup() {
        let deadline = [state(500, 1000, 7), 0u32.to_le_bytes().to_vec()].concat();
        assert_eq!(
            local_clock_wait_decode(&deadline, 400, 1000, 7)
                .unwrap()
                .reason,
            0
        );
        let wake = [state(500, 1000, 8), 1u32.to_le_bytes().to_vec()].concat();
        assert_eq!(
            local_clock_wait_decode(&wake, 600, 1000, 7).unwrap().reason,
            1
        );
        for length in 0..36 {
            assert_eq!(
                local_clock_wait_decode(&deadline[..length], 400, 1000, 7),
                Err(-3)
            );
        }
        assert_eq!(
            local_clock_wait_decode(&[deadline, vec![0]].concat(), 400, 1000, 7),
            Err(-3)
        );
        for (counter, frequency, generation, reason) in [
            (399, 1000, 7, 0u32),
            (500, 999, 7, 0),
            (500, 1000, 6, 1),
            (500, 1000, 8, 0),
            (500, 1000, 7, 1),
            (500, 1000, 7, 2),
        ] {
            let value = [
                state(counter, frequency, generation),
                reason.to_le_bytes().to_vec(),
            ]
            .concat();
            assert_eq!(local_clock_wait_decode(&value, 400, 1000, 7), Err(-3));
        }
    }
}
