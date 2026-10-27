//! C ABI for native engines. Each function calls the same host service the
//! Wasm imports use. Nothing here is specific to one game.
//!
//! Pointer arguments follow `include/gamepkg_host.h`. Callers are C, so these
//! exports stay safe Rust functions and check each pointer before use.
#![allow(clippy::not_unsafe_ptr_arg_deref)]
use crate::audio_effects::Runtime as Effects;
use crate::audio_frame;
use crate::drawing::Frame;
use crate::frame_clock::Clock;
use crate::input_events::Queue;
use crate::network::Service as Network;
use crate::saves::MAX_SAVE;
use crate::user_files::Service as Files;
use crate::window_control::Service as Window;
use std::collections::BTreeMap;
use std::sync::Mutex;

const LOG_LIMIT: usize = 1024;
const READ_LIMIT: usize = 65536;

struct Session {
    clock: Clock,
    window: Window,
    files: Files,
    input: Queue,
    network: Network,
    effects: Option<Effects>,
    frame: Frame,
    audio: Vec<i16>,
    audio_channels: u32,
    audio_allowed: bool,
    roms: Vec<Vec<u8>>,
    assets: BTreeMap<String, Vec<u8>>,
    save: Option<Vec<u8>>,
    achievements: Vec<String>,
    pending_achievements: Vec<String>,
    log: Vec<String>,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

fn lock() -> std::sync::MutexGuard<'static, Option<Session>> {
    SESSION.lock().unwrap_or_else(|poison| poison.into_inner())
}

pub fn bind_for_test(audio: bool, input: bool, save: bool) {
    let mut files = Files::new(Vec::new());
    files.enable();
    *lock() = Some(Session {
        clock: Clock::default(),
        window: Window::new(true),
        files,
        input: Queue::new(input),
        network: Network::new(crate::manifest::Permissions {
            files: Vec::new(),
            input,
            audio,
            save,
            network: false,
            network_hosts: Vec::new(),
            network_listen: Vec::new(),
            network_allow_local: false,
        }),
        effects: None,
        frame: Frame::default(),
        audio: Vec::new(),
        audio_channels: 0,
        audio_allowed: audio,
        roms: Vec::new(),
        assets: BTreeMap::new(),
        save: save.then(Vec::new),
        achievements: vec!["first".into()],
        pending_achievements: Vec::new(),
        log: Vec::new(),
    });
}

pub fn unbind() {
    *lock() = None;
}

pub fn push_input(event: crate::input_events::Event) -> bool {
    lock()
        .as_mut()
        .is_some_and(|session| session.input.push(event))
}

pub fn logged_lines() -> Vec<String> {
    lock()
        .as_ref()
        .map(|session| session.log.clone())
        .unwrap_or_default()
}

fn slice_from<'a, T>(ptr: *const T, len: u32) -> Option<&'a [T]> {
    if len == 0 {
        Some(&[])
    } else if ptr.is_null() {
        None
    } else {
        Some(unsafe { std::slice::from_raw_parts(ptr, len as usize) })
    }
}

fn with_session<T>(f: impl FnOnce(&mut Session) -> T, denied: T) -> T {
    match lock().as_mut() {
        Some(session) => f(session),
        None => denied,
    }
}

fn copy_out(bytes: &[u8], out: *mut u8, capacity: u32) -> i32 {
    if out.is_null() && !bytes.is_empty() {
        return -1;
    }
    if bytes.len() > capacity as usize {
        return -1;
    }
    if !bytes.is_empty() {
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    }
    bytes.len() as i32
}

#[no_mangle]
pub extern "C" fn gamepkg_effect_call(
    operation: u32,
    path: *const libc::c_char,
    value: u32,
) -> i32 {
    with_session(
        |session| {
            let Some(effects) = session.effects.as_mut() else {
                return 0;
            };
            if path.is_null() {
                return -1;
            }
            let text = unsafe { std::ffi::CStr::from_ptr(path) };
            let Ok(text) = text.to_str() else {
                return -1;
            };
            i32::from(effects.call(operation, text, value))
        },
        0,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_frame_time_us() -> u64 {
    with_session(|session| session.clock.micros(), 0)
}

#[no_mangle]
pub extern "C" fn gamepkg_host_begin_frame() {
    if let Some(session) = lock().as_mut() {
        session.clock.begin();
    }
}

#[no_mangle]
pub extern "C" fn gamepkg_window_state() -> u32 {
    with_session(|session| session.window.observation(), 0)
}

#[no_mangle]
pub extern "C" fn gamepkg_window_control(operation: u32, value: u32) -> i32 {
    with_session(
        |session| {
            session.window.begin(true);
            let accepted = session.window.request(operation, value);
            session.window.finish(accepted);
            i32::from(accepted)
        },
        0,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_file_call(
    operation: u32,
    request: *const u8,
    request_len: u32,
    out: *mut u8,
    capacity: u32,
) -> i32 {
    with_session(
        |session| {
            if request_len > crate::user_files::MAX_REQUEST as u32
                || request.is_null() && request_len > 0
            {
                return -1;
            }
            let bytes = if request_len == 0 {
                &[]
            } else {
                unsafe { std::slice::from_raw_parts(request, request_len as usize) }
            };
            match session.files.call(operation, bytes, capacity as usize) {
                Some(reply) => copy_out(&reply, out, capacity),
                None => -1,
            }
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_input_events(out: *mut u8, capacity: u32) -> i32 {
    with_session(
        |session| match session.input.read(capacity as usize) {
            Some(bytes) => copy_out(&bytes, out, capacity),
            None => -1,
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_random_u64() -> u64 {
    crate::random::value().unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn gamepkg_network_call(
    operation: u32,
    request: *const u8,
    request_len: u32,
    out: *mut u8,
    capacity: u32,
) -> i32 {
    with_session(
        |session| {
            if request_len > 65540 || capacity > READ_LIMIT as u32 || operation > 9 {
                return -1;
            }
            let Some(bytes) = slice_from(request, request_len) else {
                return -1;
            };
            let reply = session
                .network
                .call(operation as u8, bytes, capacity as usize);
            match reply.first() {
                Some(0) => copy_out(&reply[1..], out, capacity),
                Some(2) => -2,
                Some(3) => -3,
                _ => -1,
            }
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_rom_count() -> u32 {
    with_session(|session| session.roms.len() as u32, 0)
}

#[no_mangle]
pub extern "C" fn gamepkg_rom_size_at(index: u32) -> i32 {
    with_session(
        |session| {
            session
                .roms
                .get(index as usize)
                .map(|rom| rom.len() as i32)
                .unwrap_or(-1)
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_rom_read_at(index: u32, offset: u32, out: *mut u8, length: u32) -> i32 {
    read_slice(
        |session| session.roms.get(index as usize).map(Vec::as_slice),
        offset,
        out,
        length,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_achievement_unlock(index: u32) -> i32 {
    with_session(
        |session| {
            let Some(id) = session.achievements.get(index as usize).cloned() else {
                return 0;
            };
            if !session.pending_achievements.contains(&id) {
                session.pending_achievements.push(id);
            }
            1
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_asset_read(
    path: *const libc::c_char,
    offset: u32,
    out: *mut u8,
    length: u32,
) -> i32 {
    with_session(
        |session| {
            if path.is_null()
                || !(1..=1024).contains(&unsafe { libc::strlen(path) })
                || length > READ_LIMIT as u32
            {
                return -1;
            }
            let text = unsafe { std::ffi::CStr::from_ptr(path) };
            let Ok(name) = text.to_str() else {
                return -1;
            };
            if !crate::archive::valid_path(name) {
                return -1;
            }
            let Some(bytes) = session.assets.get(name) else {
                return -1;
            };
            let Some(slice) = bytes.get(offset as usize..) else {
                return -1;
            };
            copy_out(&slice[..slice.len().min(length as usize)], out, length)
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_rom_size() -> i32 {
    gamepkg_rom_size_at(0)
}

#[no_mangle]
pub extern "C" fn gamepkg_rom_read(offset: u32, out: *mut u8, length: u32) -> i32 {
    gamepkg_rom_read_at(0, offset, out, length)
}

#[no_mangle]
pub extern "C" fn gamepkg_save_size() -> i32 {
    with_session(
        |session| {
            session
                .save
                .as_ref()
                .map(|save| save.len() as i32)
                .unwrap_or(-1)
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_save_read(offset: u32, out: *mut u8, length: u32) -> i32 {
    read_slice(|session| session.save.as_deref(), offset, out, length)
}

#[no_mangle]
pub extern "C" fn gamepkg_save_write(bytes: *const u8, length: u32) -> i32 {
    with_session(
        |session| {
            if session.save.is_none() || length as usize > MAX_SAVE {
                return -1;
            }
            let Some(data) = slice_from(bytes, length) else {
                return -1;
            };
            session.save = Some(data.to_vec());
            0
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_draw_clear(rgba: u32) -> i32 {
    with_session(
        |session| {
            session.frame.clear(rgba);
            0
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_draw_clip(x: f32, y: f32, width: f32, height: f32) -> i32 {
    with_session(
        |session| {
            if session.frame.clip(Some([x, y, width, height])) {
                0
            } else {
                -1
            }
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_draw_clip_reset() -> i32 {
    with_session(
        |session| {
            if session.frame.clip(None) {
                0
            } else {
                -1
            }
        },
        -1,
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_pixels(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    pixel_width: u32,
    pixel_height: u32,
    pixels: *const u8,
    length: u32,
    linear: bool,
) -> i32 {
    with_session(
        |session| {
            let Some(bytes) = crate::drawing::image::Image::byte_length(pixel_width, pixel_height)
            else {
                return -1;
            };
            if pixels.is_null()
                || length as usize != bytes
                || !session.frame.can_image(x, y, width, height, bytes)
            {
                return -1;
            }
            let slice = unsafe { std::slice::from_raw_parts(pixels, bytes) };
            let Some(image) = crate::drawing::image::Image::new(pixel_width, pixel_height, slice)
            else {
                return -1;
            };
            let accepted = if linear {
                session.frame.linear_image(x, y, width, height, image)
            } else {
                session.frame.image(x, y, width, height, image)
            };
            if accepted {
                0
            } else {
                -1
            }
        },
        -1,
    )
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn gamepkg_draw_image(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    pixel_width: u32,
    pixel_height: u32,
    pixels: *const u8,
    length: u32,
) -> i32 {
    draw_pixels(
        x,
        y,
        width,
        height,
        pixel_width,
        pixel_height,
        pixels,
        length,
        false,
    )
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn gamepkg_draw_image_linear(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    pixel_width: u32,
    pixel_height: u32,
    pixels: *const u8,
    length: u32,
) -> i32 {
    draw_pixels(
        x,
        y,
        width,
        height,
        pixel_width,
        pixel_height,
        pixels,
        length,
        true,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_draw_rect(x: f32, y: f32, width: f32, height: f32, rgba: u32) -> i32 {
    with_session(
        |session| {
            if session.frame.rect(x, y, width, height, rgba) {
                0
            } else {
                -1
            }
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_draw_circle(x: f32, y: f32, radius: f32, rgba: u32) -> i32 {
    with_session(
        |session| {
            if session.frame.circle(x, y, radius, rgba) {
                0
            } else {
                -1
            }
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_audio_submit(samples: *const i16, count: u32) -> i32 {
    submit_audio(samples, count, 1)
}

#[no_mangle]
pub extern "C" fn gamepkg_audio_submit_stereo(samples: *const i16, count: u32) -> i32 {
    submit_audio(samples, count, 2)
}

fn submit_audio(samples: *const i16, count: u32, channels: u32) -> i32 {
    with_session(
        |session| {
            if count > 1600 {
                return -1;
            }
            let Some(data) = slice_from(samples, count) else {
                return -1;
            };
            if audio_frame::append(
                &mut session.audio,
                &mut session.audio_channels,
                data,
                channels,
                session.audio_allowed,
            ) {
                0
            } else {
                -1
            }
        },
        -1,
    )
}

#[no_mangle]
pub extern "C" fn gamepkg_host_log(text: *const libc::c_char, length: u32) -> i32 {
    if text.is_null() || length == 0 || length as usize > LOG_LIMIT {
        return -1;
    }
    let bytes = unsafe { std::slice::from_raw_parts(text as *const u8, length as usize) };
    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        return -1;
    }
    let line = unsafe { std::str::from_utf8_unchecked(bytes) }.to_owned();
    if let Some(session) = lock().as_mut() {
        if session.log.len() == 32 {
            session.log.remove(0);
        }
        session.log.push(line);
    }
    0
}

fn read_slice(
    pick: impl FnOnce(&Session) -> Option<&[u8]>,
    offset: u32,
    out: *mut u8,
    length: u32,
) -> i32 {
    with_session(
        |session| {
            if length > READ_LIMIT as u32 {
                return -1;
            }
            let Some(bytes) = pick(session).and_then(|bytes| bytes.get(offset as usize..)) else {
                return -1;
            };
            copy_out(&bytes[..bytes.len().min(length as usize)], out, length)
        },
        -1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    #[test]
    fn unbound_calls_fail_closed_and_log_checks_utf8() {
        let _guard = session_lock();
        unbind();
        assert_eq!(gamepkg_frame_time_us(), 0);
        assert_eq!(gamepkg_window_control(0, 0), 0);
        assert_eq!(gamepkg_save_size(), -1);
        assert_eq!(gamepkg_audio_submit(std::ptr::null(), 0), -1);
        assert_eq!(gamepkg_host_log(std::ptr::null(), 1), -1);
        assert_eq!(
            gamepkg_host_log(b"ok".as_ptr() as *const libc::c_char, 2),
            0
        );
        let bad = [0xff, 0x00];
        assert_eq!(gamepkg_host_log(bad.as_ptr() as *const libc::c_char, 1), -1);
    }

    #[test]
    fn bound_session_uses_the_same_services_as_the_wasm_host() {
        let _guard = session_lock();
        bind_for_test(true, true, true);
        gamepkg_host_begin_frame();
        assert_eq!(gamepkg_frame_time_us(), 0);
        assert_eq!(gamepkg_window_control(2, 0), 1);
        assert_eq!(gamepkg_window_control(2, 1), 0);
        assert_eq!(gamepkg_draw_clear(0xff), 0);
        assert_eq!(gamepkg_draw_clip(0.0, 0.0, 10.0, 10.0), 0);
        assert_eq!(gamepkg_draw_clip_reset(), 0);
        assert_eq!(gamepkg_window_state(), 0);
        assert_eq!(gamepkg_effect_call(0, std::ptr::null(), 0), 0);
        let mut scratch = [0u8; 8];
        assert_eq!(
            gamepkg_file_call(0, std::ptr::null(), 0, scratch.as_mut_ptr(), 4),
            -1
        );
        lock().as_mut().unwrap().roms = vec![b"rom-bytes".to_vec()];
        lock()
            .as_mut()
            .unwrap()
            .assets
            .insert("note.txt".into(), b"asset".to_vec());
        assert_eq!(gamepkg_rom_count(), 1);
        assert_eq!(gamepkg_rom_size(), 9);
        assert_eq!(gamepkg_rom_size_at(1), -1);
        assert_eq!(gamepkg_rom_read(0, scratch.as_mut_ptr(), 4), 4);
        assert_eq!(&scratch[..4], b"rom-");
        assert_eq!(gamepkg_rom_read_at(0, 4, scratch.as_mut_ptr(), 5), 5);
        let path = std::ffi::CString::new("note.txt").unwrap();
        assert_eq!(
            gamepkg_asset_read(path.as_ptr(), 0, scratch.as_mut_ptr(), 5),
            5
        );
        assert_eq!(&scratch[..5], b"asset");
        assert_eq!(
            gamepkg_asset_read(std::ptr::null(), 0, scratch.as_mut_ptr(), 0),
            -1
        );
        assert_eq!(gamepkg_draw_rect(1281.0, 0.0, 1.0, 1.0, 0), -1);
        assert_eq!(gamepkg_draw_rect(0.0, 0.0, 1.0, 1.0, 0), 0);
        assert_eq!(gamepkg_draw_circle(10.0, 10.0, 4.0, 0xffffffff), 0);
        let pixel = [1u8, 2, 3, 4];
        assert_eq!(
            gamepkg_draw_image(0.0, 0.0, 1.0, 1.0, 1, 1, pixel.as_ptr(), 4),
            0
        );
        assert_eq!(
            gamepkg_draw_image_linear(0.0, 0.0, 1.0, 1.0, 1, 1, pixel.as_ptr(), 3),
            -1
        );
        let samples = [1i16, -1, 2, -2];
        assert_eq!(gamepkg_audio_submit(samples.as_ptr(), 4), 0);
        assert_eq!(gamepkg_audio_submit_stereo(samples.as_ptr(), 4), -1);
        let bytes = b"saved";
        assert_eq!(gamepkg_save_write(bytes.as_ptr(), bytes.len() as u32), 0);
        let mut out = [0u8; 8];
        assert_eq!(gamepkg_save_read(0, out.as_mut_ptr(), 8), 5);
        assert_eq!(&out[..5], b"saved");
        assert_eq!(
            gamepkg_save_write(bytes.as_ptr(), (MAX_SAVE + 1) as u32),
            -1
        );
        assert!(push_input(crate::input_events::Event {
            kind: 0,
            x: 0.0,
            y: 0.0,
            code: 0,
        }));
        assert_eq!(gamepkg_input_events(out.as_mut_ptr(), 16), 16);
        assert_eq!(gamepkg_input_events(out.as_mut_ptr(), 15), -1);
        assert_eq!(
            gamepkg_network_call(0, std::ptr::null(), 0, out.as_mut_ptr(), 16),
            -1
        );
        assert_eq!(gamepkg_achievement_unlock(0), 1);
        assert_eq!(gamepkg_achievement_unlock(3), 0);
        assert_ne!(gamepkg_random_u64(), gamepkg_random_u64() ^ u64::MAX);
        let line = "engine ready";
        assert_eq!(
            gamepkg_host_log(line.as_ptr() as *const libc::c_char, line.len() as u32),
            0
        );
        assert!(logged_lines().iter().any(|entry| entry == "engine ready"));
        unbind();
    }
}
