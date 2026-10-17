//! Portable, bounded frame bridge. The session owner must enforce pipe deadlines.
use anyhow::{ensure, Result};
use std::io::Read;

// Upstream SCR_MAXW/SCR_MAXH: protocol validation, not a render-quality cap.
pub const MAX_WIDTH: u32 = 10000;
pub const MAX_HEIGHT: u32 = 10000;
pub const MAX_AUDIO_SAMPLES: u32 = 4800;
pub const MAX_BUTTONS: u32 = (1 << 18) - 1;
pub const MAX_MOUSE_DELTA: i32 = 2048;
pub const MAX_EVENTS: usize = 64;
pub const MAX_WHEEL_STEPS: i32 = 16;

pub fn resize_event(width: u32, height: u32) -> Result<Vec<u8>> {
    ensure!(
        (320..=MAX_WIDTH).contains(&width) && (200..=MAX_HEIGHT).contains(&height),
        "Cube 2 resolution exceeds its bounds"
    );
    let mut packet = b"GPD1".to_vec();
    for n in [width, height, 0] {
        packet.extend(n.to_le_bytes());
    }
    Ok(packet)
}

pub fn mouse_button_event(button: u32, pressed: bool, modifiers: u32) -> Result<Vec<u8>> {
    ensure!(
        matches!(button, 2 | 4 | 5),
        "unsupported Cube 2 mouse button"
    );
    ensure!(modifiers & !0x0fc3 == 0, "unsupported Cube 2 modifiers");
    let mut bytes = b"GPC1".to_vec();
    for n in [button, u32::from(pressed), modifiers] {
        bytes.extend(n.to_le_bytes());
    }
    Ok(bytes)
}
pub fn wheel_event(x: i32, y: i32, modifiers: u32) -> Result<Vec<u8>> {
    ensure!(
        x.unsigned_abs() as u64 + y.unsigned_abs() as u64 <= MAX_WHEEL_STEPS as u64,
        "Cube 2 wheel movement exceeds its event bound"
    );
    ensure!(modifiers & !0x0fc3 == 0, "unsupported Cube 2 modifiers");
    let mut bytes = b"GPW1".to_vec();
    bytes.extend(x.to_le_bytes());
    bytes.extend(y.to_le_bytes());
    bytes.extend(modifiers.to_le_bytes());
    Ok(bytes)
}
pub const MAX_SAVE_TREE: usize = 32 * 1048576;
pub const MAX_SAVE_FILE: usize = 16 * 1048576;
pub const MAX_SAVE_FILES: usize = 256;

/// Only original-engine preferences, maps, their companions and demos persist.
pub fn save_path(path: &str) -> bool {
    if path.is_empty()
        || path.len() > 240
        || !path
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(&c))
    {
        return false;
    }
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() > 6
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == ".." || p.starts_with('.'))
    {
        return false;
    }
    matches!(path, "config.cfg" | "init.cfg" | "servers.cfg")
        || (path.starts_with("packages/")
            && parts.len() >= 3
            && [".ogz", ".cfg", ".BAK", ".jpg", ".png"]
                .iter()
                .any(|s| path.ends_with(s)))
        || ((path.starts_with("demo/") || path.starts_with("demos/")) && path.ends_with(".dmo"))
}

#[derive(Debug)]
pub struct SaveFile<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
}

/// Validate all records before any host commit or engine restoration.
pub fn save_files(bytes: &[u8]) -> Result<Vec<SaveFile<'_>>> {
    ensure!(
        (16..=MAX_SAVE_TREE).contains(&bytes.len()) && &bytes[..4] == b"GPS2",
        "invalid Cube 2 save tree"
    );
    let number =
        |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    let count = number(4);
    ensure!(
        count <= MAX_SAVE_FILES && number(8) == bytes.len() - 16 && number(12) == 0,
        "invalid Cube 2 save tree header"
    );
    let mut offset = 16;
    let mut files = Vec::with_capacity(count);
    let mut previous = "";
    for _ in 0..count {
        ensure!(bytes.len() - offset >= 16, "truncated Cube 2 save entry");
        let name = number(offset);
        let size = number(offset + 4);
        ensure!(
            (1..=240).contains(&name)
                && size <= MAX_SAVE_FILE
                && number(offset + 8) == 0
                && number(offset + 12) == 0,
            "invalid Cube 2 save entry"
        );
        offset += 16;
        ensure!(name <= bytes.len() - offset, "truncated Cube 2 save path");
        let path = std::str::from_utf8(&bytes[offset..offset + name])?;
        ensure!(
            save_path(path) && path > previous,
            "invalid, duplicate or unsorted Cube 2 save path"
        );
        ensure!(
            !files.iter().any(|f: &SaveFile<'_>| path
                .strip_prefix(f.path)
                .is_some_and(|suffix| suffix.starts_with('/'))),
            "Cube 2 save file cannot also be a directory"
        );
        previous = path;
        offset += name;
        ensure!(size <= bytes.len() - offset, "truncated Cube 2 save file");
        files.push(SaveFile {
            path,
            bytes: &bytes[offset..offset + size],
        });
        offset += size;
    }
    ensure!(offset == bytes.len(), "trailing Cube 2 save data");
    Ok(files)
}

/// SDL's documented USB scancodes, never native event structs or pointers.
pub fn key_event(scancode: u32, pressed: bool, repeat: bool, modifiers: u32) -> Result<Vec<u8>> {
    ensure!((4..=231).contains(&scancode), "unsupported Cube 2 key");
    ensure!(modifiers & !0x0fc3 == 0, "unsupported Cube 2 modifiers");
    let mut bytes = b"GPI1".to_vec();
    for n in [
        scancode,
        u32::from(pressed) | (u32::from(repeat) << 1),
        modifiers,
    ] {
        bytes.extend(n.to_le_bytes());
    }
    Ok(bytes)
}
pub fn text_event(text: &str) -> Result<Vec<u8>> {
    ensure!(
        !text.is_empty() && text.len() <= 31 && !text.contains('\0'),
        "invalid Cube 2 text event"
    );
    let mut bytes = b"GPT1".to_vec();
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend([0; 8]);
    bytes.extend(text.as_bytes());
    Ok(bytes)
}
pub const RESET_INPUT: [u8; 16] = *b"GPR1\0\0\0\0\0\0\0\0\0\0\0\0";
pub fn modifiers_event(modifiers: u32) -> Result<Vec<u8>> {
    ensure!(modifiers & !0x0fc3 == 0, "unsupported Cube 2 modifiers");
    let mut bytes = b"GPM1".to_vec();
    bytes.extend(modifiers.to_le_bytes());
    bytes.extend([0; 8]);
    Ok(bytes)
}

#[derive(Debug)]
pub enum Update {
    Frame(Frame),
    Quit(Vec<u8>),
}

/// Only an explicit quit event with a valid bounded snapshot is a normal exit.
pub fn read_update(mut reader: impl Read) -> Result<Update> {
    let mut magic = [0; 4];
    reader.read_exact(&mut magic)?;
    if &magic == b"GPQ1" {
        return Ok(Update::Quit(read_save(reader)?));
    }
    ensure!(&magic == b"GPF2", "invalid Cube 2 update magic");
    Ok(Update::Frame(read_frame(
        std::io::Cursor::new(magic).chain(reader),
    )?))
}

/// Encode defined controls only; never serialize a native struct or pointer.
pub fn input(buttons: u32, mouse_x: i32, mouse_y: i32) -> Result<[u8; 16]> {
    ensure!(buttons & !MAX_BUTTONS == 0, "unknown Cube 2 input buttons");
    ensure!(
        (-MAX_MOUSE_DELTA..=MAX_MOUSE_DELTA).contains(&mouse_x)
            && (-MAX_MOUSE_DELTA..=MAX_MOUSE_DELTA).contains(&mouse_y),
        "Cube 2 mouse movement exceeds the per-frame bound"
    );
    let mut packet = [0; 16];
    packet[..4].copy_from_slice(b"GPK1");
    packet[4..8].copy_from_slice(&buttons.to_le_bytes());
    packet[8..12].copy_from_slice(&mouse_x.to_le_bytes());
    packet[12..16].copy_from_slice(&mouse_y.to_le_bytes());
    Ok(packet)
}

#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// Top-down opaque RGBA, suitable for the bounded host image API.
    pub rgba: Vec<u8>,
    /// Bounded mono 48 kHz signed PCM, with explicit little-endian wire encoding.
    pub audio: Vec<i16>,
}

impl Frame {
    /// Present through existing bounded images without raising guest image limits.
    pub fn drawing_frame(&self) -> Result<crate::drawing::Frame> {
        ensure!(
            (1..=MAX_WIDTH).contains(&self.width)
                && (1..=MAX_HEIGHT).contains(&self.height)
                && self.rgba.len() == self.width as usize * self.height as usize * 4,
            "invalid Cube 2 frame pixels"
        );
        let mut frame = crate::drawing::Frame::default();
        frame.background = Some(0x000000ff);
        let tile = crate::drawing::image::MAX_DIMENSION as usize;
        let (width, height) = (self.width as usize, self.height as usize);
        for y in (0..height).step_by(tile) {
            for x in (0..width).step_by(tile) {
                let (w, h) = ((width - x).min(tile), (height - y).min(tile));
                let mut pixels = Vec::with_capacity(w * h * 4);
                for row in y..y + h {
                    let start = (row * width + x) * 4;
                    pixels.extend_from_slice(&self.rgba[start..start + w * 4]);
                }
                let image = crate::drawing::image::Image::new(w as u32, h as u32, &pixels)
                    .expect("bounded image tile");
                ensure!(
                    frame.image(
                        x as f32 * 640.0 / width as f32,
                        y as f32 * 480.0 / height as f32,
                        w as f32 * 640.0 / width as f32,
                        h as f32 * 480.0 / height as f32,
                        image,
                    ),
                    "Cube 2 drawing frame exceeded host limits"
                );
            }
        }
        Ok(frame)
    }
}

/// Validate the complete header before allocating or reading a variable payload.
pub fn read_frame(mut reader: impl Read) -> Result<Frame> {
    let mut header = [0; 20];
    reader.read_exact(&mut header)?;
    ensure!(&header[..4] == b"GPF2", "invalid Cube 2 frame magic");
    let number =
        |offset| u32::from_le_bytes(header[offset..offset + 4].try_into().expect("fixed header"));
    let (width, height, length) = (number(4), number(8), number(12));
    ensure!(
        (1..=MAX_WIDTH).contains(&width) && (1..=MAX_HEIGHT).contains(&height),
        "Cube 2 frame dimensions exceed the bound"
    );
    let samples = number(16);
    ensure!(
        samples <= MAX_AUDIO_SAMPLES,
        "Cube 2 audio frame exceeds the bound"
    );
    let expected = width
        .checked_mul(height)
        .and_then(|n| n.checked_mul(4))
        .expect("bounded dimensions");
    ensure!(length == expected, "invalid Cube 2 frame byte length");
    let mut rgba = vec![0; expected as usize];
    reader.read_exact(&mut rgba)?;
    let mut pcm = vec![0; samples as usize * 2];
    reader.read_exact(&mut pcm)?;
    let audio = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .map(|s| i16::from_le_bytes([s[0], s[1]]))
        .collect();
    let stride = width as usize * 4;
    for y in 0..height as usize / 2 {
        let (top, bottom) = rgba.split_at_mut((height as usize - 1 - y) * stride);
        top[y * stride..(y + 1) * stride].swap_with_slice(&mut bottom[..stride]);
    }
    for pixel in rgba.as_chunks_mut::<4>().0 {
        pixel[3] = 255;
    }
    Ok(Frame {
        width,
        height,
        rgba,
        audio,
    })
}

/// Restore only the three fixed configuration slots; no filename is accepted.
pub fn save_seed(saved: &[u8]) -> Result<Vec<u8>> {
    if saved.starts_with(b"GPS2") {
        save_files(saved)?;
        let mut seed = saved.to_vec();
        seed[..4].copy_from_slice(b"GPB2");
        return Ok(seed);
    }
    if saved.is_empty() {
        let mut bytes = vec![0; 16];
        bytes[..4].copy_from_slice(b"GPB1");
        return Ok(bytes);
    }
    ensure!(
        saved.len() <= crate::saves::MAX_SAVE && saved.len() >= 16,
        "invalid Cube 2 configuration size"
    );
    let size = save_payload_size(&saved[..16])?;
    ensure!(
        saved.len() == size + 16,
        "invalid Cube 2 configuration length"
    );
    let mut bytes = saved.to_vec();
    bytes[..4].copy_from_slice(b"GPB1");
    Ok(bytes)
}
fn save_payload_size(header: &[u8]) -> Result<usize> {
    ensure!(
        header.len() == 16 && &header[..4] == b"GPS1",
        "invalid Cube 2 configuration header"
    );
    let mut total = 0usize;
    for offset in [4, 8, 12] {
        let length =
            u32::from_le_bytes(header[offset..offset + 4].try_into().expect("fixed header"));
        ensure!(
            length as usize <= crate::saves::MAX_SAVE - 16 - total,
            "Cube 2 configuration exceeds its save quota"
        );
        total += length as usize;
    }
    Ok(total)
}
pub fn read_save(mut reader: impl Read) -> Result<Vec<u8>> {
    let mut header = [0; 16];
    reader.read_exact(&mut header)?;
    let tree = &header[..4] == b"GPS2";
    let size = if tree {
        let count = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        ensure!(
            count <= MAX_SAVE_FILES && size <= MAX_SAVE_TREE - 16 && header[12..] == [0; 4],
            "invalid Cube 2 save tree header"
        );
        size
    } else {
        save_payload_size(&header)?
    };
    let mut bytes = vec![0; size + 16];
    bytes[..16].copy_from_slice(&header);
    reader.read_exact(&mut bytes[16..])?;
    if tree {
        save_files(&bytes)?;
    }
    Ok(bytes)
}
pub const SAVE_REQUEST: [u8; 16] = *b"GPS1\0\0\0\0\0\0\0\0\0\0\0\0";
pub const CLOSE_REQUEST: [u8; 16] = *b"GPE1\0\0\0\0\0\0\0\0\0\0\0\0";

#[cfg(test)]
pub(crate) fn test_tree(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = b"GPS2".to_vec();
    bytes.extend((files.len() as u32).to_le_bytes());
    bytes.extend([0; 8]);
    for (path, data) in files {
        bytes.extend((path.len() as u32).to_le_bytes());
        bytes.extend((data.len() as u32).to_le_bytes());
        bytes.extend([0; 8]);
        bytes.extend(path.as_bytes());
        bytes.extend(*data);
    }
    let size = (bytes.len() - 16) as u32;
    bytes[8..12].copy_from_slice(&size.to_le_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    #[test]
    fn resize_packets_are_portable_and_bounded() {
        let p = super::resize_event(1280, 720).unwrap();
        assert_eq!(&p[..4], b"GPD1");
        assert_eq!(&p[4..8], &1280u32.to_le_bytes());
        assert_eq!(&p[8..12], &720u32.to_le_bytes());
        assert_eq!(&p[12..], &[0; 4]);
        for (w, h) in [
            (0, 480),
            (319, 480),
            (640, 199),
            (10001, 720),
            (1280, 10001),
            (u32::MAX, 480),
        ] {
            assert!(super::resize_event(w, h).is_err());
        }
    }
    use super::*;
    fn header(width: u32, height: u32, length: u32) -> Vec<u8> {
        let mut out = b"GPF2".to_vec();
        for n in [width, height, length] {
            out.extend(n.to_le_bytes());
        }
        out.extend(0u32.to_le_bytes());
        out
    }
    #[test]
    fn pointer_packets_have_strict_types_flags_and_signed_bounds() {
        assert_eq!(
            mouse_button_event(4, true, 65).unwrap(),
            b"GPC1\x04\0\0\0\x01\0\0\0\x41\0\0\0"
        );
        assert_eq!(
            wheel_event(-2, 3, 0).unwrap(),
            b"GPW1\xfe\xff\xff\xff\x03\0\0\0\0\0\0\0"
        );
        for button in [0, 1, 3, 6, u32::MAX] {
            assert!(mouse_button_event(button, true, 0).is_err());
        }
        for (x, y) in [(17, 0), (-17, 0), (9, 8), (i32::MIN, 0), (0, i32::MAX)] {
            assert!(wheel_event(x, y, 0).is_err());
        }
        assert!(wheel_event(-8, 8, 0).is_ok());
        assert!(wheel_event(0, 0, 0).is_ok());
        assert!(wheel_event(0, 1, 1 << 31).is_err());
        assert!(mouse_button_event(2, false, 1 << 31).is_err());
    }
    #[test]
    fn saved_trees_are_bounded_canonical_and_accept_only_game_files() {
        let tree = test_tree(&[
            ("config.cfg", b"name test"),
            ("demos/one.dmo", b"demo"),
            ("packages/custom/map.ogz", b"map"),
        ]);
        let files = save_files(&tree).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[2].bytes, b"map");
        assert_eq!(read_save(tree.as_slice()).unwrap(), tree);
        assert_eq!(&save_seed(&tree).unwrap()[..4], b"GPB2");
        for length in 0..tree.len() {
            assert!(save_files(&tree[..length]).is_err());
        }
        for offset in [4, 8, 12, 16, 20, 24, 28] {
            let mut bad = tree.clone();
            bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(
                save_files(&bad).is_err(),
                "accepted malformed field {offset}"
            );
            assert!(read_save(bad.as_slice()).is_err());
        }
        for files in [
            vec![("config.cfg", b"".as_slice()), ("config.cfg", b"x")],
            vec![("init.cfg", b"".as_slice()), ("config.cfg", b"x")],
            vec![
                ("packages/base/a.cfg", b"".as_slice()),
                ("packages/base/a.cfg/sub.ogz", b"x"),
            ],
        ] {
            assert!(save_files(&test_tree(&files)).is_err());
        }
        let mut extra = tree.clone();
        extra.push(0);
        assert!(save_files(&extra).is_err());
        assert!(save_files(&vec![0; MAX_SAVE_TREE + 1]).is_err());
    }
    #[test]
    fn saved_file_paths_reject_escape_encodings_hidden_files_and_other_content() {
        for path in [
            "config.cfg",
            "init.cfg",
            "servers.cfg",
            "packages/base/my_map.ogz",
            "packages/custom/my_map_123.BAK",
            "packages/base/map.cfg",
            "packages/base/map.jpg",
            "packages/base/map.png",
            "demos/match.dmo",
        ] {
            assert!(save_path(path), "rejected {path}");
        }
        for path in [
            "",
            "/config.cfg",
            "../config.cfg",
            "packages/base/../evil.ogz",
            "packages/base/./map.ogz",
            "packages//base/map.ogz",
            "packages/base/.hidden.ogz",
            "packages/base/map.ogz/",
            "packages/base/map.ogz\0",
            "packages/base/%2e%2e/map.ogz",
            "packages/base/%252e/map.ogz",
            "packages\\base\\map.ogz",
            "C:/packages/base/map.ogz",
            "file:/packages/base/map.ogz",
            "packages/base/∕map.ogz",
            "packages/base/a:b.ogz",
            "packages/base/a?b.ogz",
            "packages/base/a*b.ogz",
            "packages/base/a\nb.ogz",
            "packages/base/a\tb.ogz",
            "packages/base/map.cpp",
            "packages/base/map.so",
            "packages/base/map.ogz.exe",
            "data/autoexec.cfg",
            "autoexec.cfg",
            "packages/map.ogz",
            "demos/../map.dmo",
            "demos/map.cfg",
            "packages/a/b/c/d/e/f.ogz",
        ] {
            assert!(!save_path(path), "accepted {path:?}");
            assert!(save_files(&test_tree(&[(path, b"x")])).is_err());
        }
    }
    #[test]
    fn keyboard_and_utf8_text_packets_are_bounded_and_portable() {
        assert_eq!(
            key_event(69, true, true, 65).unwrap(),
            b"GPI1\x45\0\0\0\x03\0\0\0\x41\0\0\0"
        );
        for code in [0, 3, 232, u32::MAX] {
            assert!(key_event(code, true, false, 0).is_err());
        }
        assert!(key_event(69, true, false, 1 << 31).is_err());
        assert!(modifiers_event(0x0fc3).is_ok());
        assert!(modifiers_event(1 << 31).is_err());
        let bytes = text_event("héllo").unwrap();
        assert_eq!(&bytes[..4], b"GPT1");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 6);
        assert_eq!(&bytes[16..], "héllo".as_bytes());
        for text in ["", "\0", "abcdefghijklmnopqrstuvwxyz012345"] {
            assert!(text_event(text).is_err());
        }
        assert!(text_event("abcdefghijklmnopqrstuvwxyz01234").is_ok());
    }
    #[test]
    fn orderly_quit_requires_a_complete_bounded_snapshot() {
        let mut bytes = b"GPQ1GPS1".to_vec();
        bytes.extend([0; 12]);
        assert!(
            matches!(read_update(bytes.as_slice()).unwrap(), Update::Quit(save) if save.len()==16)
        );
        for size in 0..bytes.len() {
            assert!(read_update(&bytes[..size]).is_err());
        }
        bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_update(bytes.as_slice()).is_err());
        assert!(read_update(b"FAIL".as_slice()).is_err());
        let mut frame = header(1, 1, 4);
        frame.extend([1, 2, 3, 255]);
        assert!(
            matches!(read_update(frame.as_slice()).unwrap(), Update::Frame(frame) if frame.rgba==[1,2,3,255])
        );
    }
    #[test]
    fn configuration_snapshot_is_fixed_slot_bounded_and_cannot_name_paths() {
        let mut snapshot = b"GPS1".to_vec();
        for length in [3u32, 2, 0] {
            snapshot.extend(length.to_le_bytes());
        }
        snapshot.extend(b"abcde");
        assert_eq!(read_save(snapshot.as_slice()).unwrap(), snapshot);
        let seed = save_seed(&snapshot).unwrap();
        assert_eq!(&seed[..4], b"GPB1");
        assert_eq!(&seed[4..], &snapshot[4..]);
        assert_eq!(save_seed(&[]).unwrap().len(), 16);
        for size in 0..snapshot.len() {
            assert!(save_seed(&snapshot[..size]).is_err() || size == 0);
        }
        let mut extra = snapshot.clone();
        extra.push(0);
        assert!(save_seed(&extra).is_err());
        for length in [crate::saves::MAX_SAVE as u32, u32::MAX] {
            let mut bad = snapshot.clone();
            bad[4..8].copy_from_slice(&length.to_le_bytes());
            assert!(read_save(bad.as_slice()).is_err());
            assert!(save_seed(&bad).is_err());
        }
    }
    #[test]
    fn controls_have_fixed_little_endian_encoding_and_strict_bounds() {
        let packet = input((1 << 16) | 1, -2, 2048).unwrap();
        assert_eq!(
            &packet,
            b"GPK1\x01\x00\x01\x00\xfe\xff\xff\xff\x00\x08\x00\x00"
        );
        for buttons in [1 << 18, 1 << 31, u32::MAX] {
            assert!(input(buttons, 0, 0).is_err());
        }
        for delta in [-2049, 2049, i32::MIN, i32::MAX] {
            assert!(input(0, delta, 0).is_err());
            assert!(input(0, 0, delta).is_err());
        }
        assert!(input(MAX_BUTTONS, -2048, 2048).is_ok());
    }
    #[test]
    fn malformed_dimensions_lengths_and_truncated_frames_are_rejected() {
        for (w, h, len) in [
            (0, 1, 0),
            (1, 0, 0),
            (10001, 1, 40004),
            (1, 10001, 40004),
            (u32::MAX, u32::MAX, 4),
            (1, 1, u32::MAX),
            (2, 2, 4),
        ] {
            assert!(read_frame(header(w, h, len).as_slice()).is_err());
        }
        for n in 0..24 {
            let mut bytes = header(1, 1, 4);
            bytes.extend([1, 2, 3, 4]);
            assert!(read_frame(&bytes[..n]).is_err());
        }
        let mut bad = header(1, 1, 4);
        bad[..4].copy_from_slice(b"FAIL");
        assert!(read_frame(bad.as_slice()).is_err());
    }
    #[test]
    fn audio_is_bounded_little_endian_pcm_and_truncation_is_rejected() {
        let mut bytes = header(1, 1, 4);
        bytes[16..20].copy_from_slice(&3u32.to_le_bytes());
        bytes.extend([1, 2, 3, 255]);
        for sample in [i16::MIN, -1, i16::MAX] {
            bytes.extend(sample.to_le_bytes());
        }
        assert_eq!(
            read_frame(bytes.as_slice()).unwrap().audio,
            [i16::MIN, -1, i16::MAX]
        );
        for length in 24..bytes.len() {
            assert!(read_frame(&bytes[..length]).is_err());
        }
        for count in [MAX_AUDIO_SAMPLES + 1, u32::MAX] {
            let mut bad = header(1, 1, 4);
            bad[16..20].copy_from_slice(&count.to_le_bytes());
            assert!(read_frame(bad.as_slice()).is_err());
        }
    }
    #[test]
    fn large_engine_frames_use_bounded_tiles_without_expanding_guest_limits() {
        let mut pixels = vec![0; 640 * 480 * 4];
        for y in 0..480 {
            for x in 0..640 {
                let offset = (y * 640 + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[
                    (x % 251) as u8,
                    (y % 251) as u8,
                    23,
                    255,
                ]);
            }
        }
        let frame = Frame {
            width: 640,
            height: 480,
            rgba: pixels,
            audio: vec![],
        };
        let drawing = frame.drawing_frame().unwrap();
        assert_eq!(drawing.commands.len(), 2);
        for (index, command) in drawing.commands.iter().enumerate() {
            let crate::drawing::Command::Image {
                x,
                y,
                width,
                height,
                image,
            } = command
            else {
                panic!("image tile expected")
            };
            let offset = index * 512;
            assert_eq!((*x, *y), (offset as f32, 0.0));
            assert_eq!((*width, *height), ((640 - offset).min(512) as f32, 480.0));
            assert_eq!(&image.pixels()[..4], &[(offset % 251) as u8, 0, 23, 255]);
        }
        assert!(Frame {
            width: u32::MAX,
            height: 480,
            rgba: vec![],
            audio: vec![],
        }
        .drawing_frame()
        .is_err());
        assert!(Frame {
            width: 640,
            height: 480,
            rgba: vec![0; 4],
            audio: vec![],
        }
        .drawing_frame()
        .is_err());
    }
    #[test]
    fn pixels_flip_rows_preserve_channels_and_force_opaque_alpha() {
        let mut bytes = header(1, 3, 12);
        bytes.extend([1, 2, 3, 0, 4, 5, 6, 10, 7, 8, 9, 20]);
        let frame = read_frame(bytes.as_slice()).unwrap();
        assert_eq!((frame.width, frame.height), (1, 3));
        assert_eq!(frame.rgba, [7, 8, 9, 255, 4, 5, 6, 255, 1, 2, 3, 255]);
    }
}
