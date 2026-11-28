use super::*;
use std::io::{Read, Write};
pub const ABI: u32 = 1;
pub const MAX_MESSAGE: usize = 16 * 1024 * 1024;
const MAX_IMAGES: usize = 1024;
const MAX_IMAGE_BYTES: usize = 128 * 1024 * 1024;

pub(super) struct Request<'a>(pub(super) &'a [u8]);
impl<'a> Request<'a> {
    fn bytes(&mut self, count: usize) -> Result<&'a [u8]> {
        let bytes = self.0.get(..count).ok_or(Error::Invalid)?;
        self.0 = &self.0[count..];
        Ok(bytes)
    }
    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    pub(super) fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
    fn float(&mut self) -> Result<f32> {
        let f = f32::from_bits(self.u32()?);
        if f.is_finite() {
            Ok(f)
        } else {
            Err(Error::Invalid)
        }
    }
    pub(super) fn end(&self) -> Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::Invalid)
        }
    }
}
fn room(capacity: usize, needed: usize) -> Result<()> {
    if capacity >= needed {
        Ok(())
    } else {
        Err(Error::Invalid)
    }
}
fn permitted(allowed: bool) -> Result<()> {
    if allowed {
        Ok(())
    } else {
        Err(Error::Denied)
    }
}

pub(super) fn dispatch<B: Backend>(
    host: &mut Session<B>,
    op: u32,
    bytes: &[u8],
    cap: usize,
) -> Result<Vec<u8>> {
    let mut r = Request(bytes);
    match op {
        0 => {
            r.end()?;
            room(cap, 4)?;
            Ok(ABI.to_le_bytes().to_vec())
        }
        1..=9 | 12..=19 | 23..=26 => file_dispatch(
            &mut *host.files.lock().map_err(|_| Error::Failed)?,
            op,
            bytes,
            cap,
        ),
        10 => {
            if host.window.closed {
                return Err(Error::Denied);
            }
            let command = match r.u32()? {
                0..=2 => {
                    let operation = u32::from_le_bytes(bytes[..4].try_into().unwrap());
                    let value = r.u32()?;
                    if value > 1 {
                        return Err(Error::Invalid);
                    }
                    match operation {
                        0 => WindowCommand::Fullscreen(value != 0),
                        1 => WindowCommand::CursorVisible(value != 0),
                        _ => WindowCommand::CursorLocked(value != 0),
                    }
                }
                3 => {
                    let w = r.u32()?;
                    let h = r.u32()?;
                    if !(1..=16384).contains(&w) || !(1..=16384).contains(&h) {
                        return Err(Error::Invalid);
                    }
                    WindowCommand::Resize(w, h)
                }
                4 => WindowCommand::Close,
                _ => return Err(Error::Invalid),
            };
            r.end()?;
            host.backend.window(command)?;
            Ok(Vec::new())
        }
        11 => {
            r.end()?;
            room(cap, 12)?;
            let s = host.window;
            let flags =
                u32::from(s.fullscreen) | (u32::from(s.focused) << 1) | (u32::from(s.closed) << 2);
            Ok([
                s.width.to_le_bytes(),
                s.height.to_le_bytes(),
                flags.to_le_bytes(),
            ]
            .concat())
        }
        20 => {
            room(cap, 8)?;
            let width = r.u32()?;
            let height = r.u32()?;
            let limit = host.backend.texture_limit().min(8192);
            if width == 0 || height == 0 || width > limit || height > limit {
                return Err(Error::Invalid);
            }
            let len = u64::from(width) * u64::from(height) * 4;
            if len != r.0.len() as u64 {
                return Err(Error::Invalid);
            }
            let id = retain_image(host, width, height, r.0)?;
            Ok(id.to_le_bytes().to_vec())
        }
        21 => {
            let id = r.u64()?;
            r.end()?;
            let image = host.images.remove(&id).ok_or(Error::Denied)?;
            host.image_bytes -= image.rgba.len();
            Ok(Vec::new())
        }
        22 | 70 => {
            if host.window.closed {
                return Err(Error::Denied);
            }
            let clear = r.u32()?;
            let count = r.u32()? as usize;
            let stride = if op == 70 { 52 } else { 32 };
            if count > 4096 || r.0.len() != count * stride {
                return Err(Error::Invalid);
            }
            let mut draws = Vec::with_capacity(count);
            for _ in 0..count {
                let source = r.u64()?;
                let rect = [r.float()?, r.float()?, r.float()?, r.float()?];
                if rect.iter().any(|v| v.abs() > 65536.0) || rect[2] <= 0.0 || rect[3] <= 0.0 {
                    return Err(Error::Invalid);
                }
                let filter = r.u32()?;
                let shader = r.u32()?;
                if filter > 1 || (shader != u32::MAX && shader >= host.backend.shader_count()) {
                    return Err(Error::Invalid);
                }
                let (clip, flags) = if op == 70 {
                    let flags = r.u32()?;
                    let clip = [r.float()?, r.float()?, r.float()?, r.float()?];
                    if flags & !15 != 0
                        || clip.iter().any(|v| v.abs() > 65536.0)
                        || clip[2] < 0.0
                        || clip[3] < 0.0
                        || (flags & 2 != 0 && rect[2] != rect[3])
                        || (flags & 4 != 0 && (flags & 10 != 0 || filter != 1))
                        || (flags & 14 != 0 && shader != u32::MAX)
                    {
                        return Err(Error::Invalid);
                    }
                    ((flags & 1 != 0).then_some(clip), flags)
                } else {
                    (None, 0)
                };
                let circle = flags & 2 != 0;
                let straight = flags & 4 != 0;
                let (image, solid) = if flags & 8 != 0 {
                    if filter != 0 {
                        return Err(Error::Invalid);
                    }
                    (
                        None,
                        Some(u32::try_from(source).map_err(|_| Error::Invalid)?),
                    )
                } else {
                    let image = host.images.get(&source).ok_or(Error::Denied)?.clone();
                    if circle && image.rgba.len() != 4 {
                        return Err(Error::Invalid);
                    }
                    (Some(image), None)
                };
                draws.push(Draw {
                    image,
                    solid,
                    rect,
                    linear: filter != 0,
                    clip,
                    circle,
                    straight,
                    shader: (shader != u32::MAX).then_some(shader),
                });
            }
            r.end()?;
            // Backend sees only a completely validated immutable frame.
            host.backend.present(Frame { clear, draws })?;
            if op == 70 {
                host.finish_tick()?;
            }
            Ok(Vec::new())
        }
        71 => {
            let operation = r.u32()?;
            let value = r.u32()?;
            r.end()?;
            room(cap, 4)?;
            let tick = host
                .tick
                .as_mut()
                .filter(|tick| tick.active)
                .ok_or(Error::Denied)?;
            Ok(u32::from(tick.window.request(operation, value))
                .to_le_bytes()
                .to_vec())
        }
        72 => {
            let operation = r.u32()?;
            let value = r.u32()?;
            room(cap, 4)?;
            permitted(
                host.permissions.audio && host.tick.as_ref().is_some_and(|tick| tick.active),
            )?;
            if r.0.len() > 1024 {
                return Err(Error::Invalid);
            }
            let name = std::str::from_utf8(r.0).map_err(|_| Error::Invalid)?;
            Ok(u32::from(host.backend.tick_audio(operation, name, value))
                .to_le_bytes()
                .to_vec())
        }
        73 => {
            let index = r.u32()?;
            r.end()?;
            room(cap, 4)?;
            let tick = host
                .tick
                .as_mut()
                .filter(|tick| tick.active)
                .ok_or(Error::Denied)?;
            Ok(u32::from(tick.achievement(index)).to_le_bytes().to_vec())
        }
        74 => {
            let operation = r.u32()?;
            let tick = host
                .tick
                .as_mut()
                .filter(|tick| tick.active)
                .ok_or(Error::Denied)?;
            tick.files.call(operation, r.0, cap).ok_or(Error::Denied)
        }
        30 => {
            permitted(host.permissions.audio)?;
            let channels = r.u32()?;
            if !matches!(channels, 1 | 2)
                || r.0.is_empty()
                || !r.0.len().is_multiple_of(channels as usize * 2)
                || r.0.len() > 1600 * channels as usize * 2
            {
                return Err(Error::Invalid);
            }
            let pcm: Vec<i16> =
                r.0.as_chunks::<2>()
                    .0
                    .iter()
                    .copied()
                    .map(i16::from_le_bytes)
                    .collect();
            host.backend.audio(&pcm, channels)?;
            Ok(Vec::new())
        }
        40 => {
            permitted(host.permissions.input)?;
            r.end()?;
            if cap == 0 || !cap.is_multiple_of(40) {
                return Err(Error::Invalid);
            }
            let mut out = Vec::new();
            let events = host
                .input
                .drain((cap / 40).min(gamepkg_native_input::MAX_EVENTS));
            if host.tick.as_ref().is_some_and(|tick| !tick.active) {
                host.begin_tick(events.iter().any(|e| matches!(e.kind, 1 | 5 | 8)))?;
            }
            for e in events {
                out.extend(e.device.to_le_bytes());
                out.extend(e.timestamp_ns.to_le_bytes());
                out.extend(e.kind.to_le_bytes());
                out.extend(e.code.to_le_bytes());
                out.extend(e.x.to_le_bytes());
                out.extend(e.y.to_le_bytes());
                out.extend(e.contact.to_le_bytes());
            }
            Ok(out)
        }
        41 => {
            permitted(host.permissions.input)?;
            let player = r.u32()?;
            r.end()?;
            room(cap, 32)?;
            if player >= gamepkg_native_input::MAX_PLAYERS as u32 {
                return Err(Error::Invalid);
            }
            let s = host.input.gamepad(player as usize).unwrap_or_default();
            let mut out = Vec::new();
            out.extend(s.device.to_le_bytes());
            out.extend(s.timestamp_ns.to_le_bytes());
            out.extend(s.buttons.to_le_bytes());
            for v in [s.left_x, s.left_y, s.right_x, s.right_y] {
                out.extend(v.to_le_bytes());
            }
            out.extend(s.left_trigger.to_le_bytes());
            out.extend(s.right_trigger.to_le_bytes());
            Ok(out)
        }
        42 => {
            permitted(host.permissions.input)?;
            let device = r.u64()?;
            let code = r.u32()?;
            r.end()?;
            room(cap, 4)?;
            let held = host.input.key(device, code).ok_or(Error::Denied)?;
            Ok(u32::from(held).to_le_bytes().to_vec())
        }
        43 => {
            permitted(host.permissions.input)?;
            let device = r.u64()?;
            r.end()?;
            room(cap, 32)?;
            let s = host.input.pointer(device).ok_or(Error::Denied)?;
            Ok([
                s.device.to_le_bytes().as_slice(),
                &s.timestamp_ns.to_le_bytes(),
                &s.x.to_le_bytes(),
                &s.y.to_le_bytes(),
                &s.buttons.to_le_bytes(),
                &0u32.to_le_bytes(),
            ]
            .concat())
        }
        44 => {
            permitted(host.permissions.input)?;
            r.end()?;
            let contacts = host.input.contacts();
            room(cap, contacts.len() * 32)?;
            let mut out = Vec::new();
            for s in contacts {
                out.extend(s.device.to_le_bytes());
                out.extend(s.timestamp_ns.to_le_bytes());
                out.extend(s.contact.to_le_bytes());
                out.extend(s.x.to_le_bytes());
                out.extend(s.y.to_le_bytes());
            }
            Ok(out)
        }
        45 => {
            permitted(host.permissions.input)?;
            let player = r.u32()?;
            let low = r.u32()?;
            let high = r.u32()?;
            let ms = r.u32()?;
            r.end()?;
            if player >= 4 || low > 65535 || high > 65535 || ms > 10000 {
                return Err(Error::Invalid);
            }
            let pad = host.input.gamepad(player as usize).ok_or(Error::Denied)?;
            if pad.device == 0 || !host.window.focused || host.window.closed {
                return Err(Error::Denied);
            }
            host.backend
                .rumble(pad.device, low as u16, high as u16, ms)?;
            Ok(Vec::new())
        }
        46 => {
            permitted(host.permissions.input)?;
            r.end()?;
            let devices = host.input.devices();
            room(cap, devices.len() * 16)?;
            let mut out = Vec::new();
            for d in devices {
                out.extend(d.device.to_le_bytes());
                out.extend(d.kind.to_le_bytes());
                out.extend(d.player.to_le_bytes());
            }
            Ok(out)
        }
        57 => {
            permitted(host.permissions.input)?;
            let player = r.u32()? as usize;
            let low = r.u32()?;
            let high = r.u32()?;
            let frame = r.u64()?;
            r.end()?;
            if player >= 4 || low > 65535 || high > 65535 || frame == 0 {
                return Err(Error::Invalid);
            }
            // A pulse belongs to exactly one numbered VI frame. Replaying or
            // reordering a frame cannot re-trigger a motor on this session.
            if host.rumble_frames[player].is_some_and(|last| frame <= last) {
                return Err(Error::Invalid);
            }
            let pad = host.input.gamepad(player).ok_or(Error::Denied)?;
            if pad.device == 0 || !host.window.focused || host.window.closed {
                return Err(Error::Denied);
            }
            host.backend.rumble_frame(
                pad.device,
                low as u16,
                high as u16,
                host.vi_cadence.period_ns,
            )?;
            host.rumble_frames[player] = Some(frame);
            Ok(Vec::new())
        }
        58 => {
            room(cap, 16)?;
            let standard = r.u32()?;
            r.end()?;
            let cadence = timing::vi_cadence(standard).ok_or(Error::Invalid)?;
            host.vi_cadence = cadence;
            Ok([
                cadence.period_ns.to_le_bytes().as_slice(),
                &cadence.standard.to_le_bytes(),
                &cadence.fields_per_frame.to_le_bytes(),
            ]
            .concat())
        }
        50 => network_dispatch(
            &mut *host.network.lock().map_err(|_| Error::Failed)?,
            host.permissions.network,
            bytes,
            cap,
        ),
        60..=62 | 65..=66 => host.timing.call(op, bytes, cap as u32, false),
        63 => {
            room(cap, 8)?;
            let level = r.u32()?;
            let code = r.u32()?;
            let count = r.u32()? as usize;
            if level > 3 || count > diagnostics::MAX_VALUES {
                return Err(Error::Invalid);
            }
            let mut values = Vec::with_capacity(count);
            for _ in 0..count {
                values.push(r.u64()?);
            }
            r.end()?;
            let (now, _) = host.timing.snapshot()?;
            let sequence = host.diagnostics.record(now, level, code, values)?;
            Ok(sequence.to_le_bytes().to_vec())
        }
        64 => {
            room(cap, 8)?;
            let level = r.u32()?;
            let code = r.u32()?;
            let count = r.u32()? as usize;
            if level > 3 || count > diagnostics::MAX_EVENT_BYTES {
                return Err(Error::Invalid);
            }
            let bytes = r.bytes(count)?;
            r.end()?;
            let (now, _) = host.timing.snapshot()?;
            let sequence = host
                .diagnostics
                .record_bytes(now, level, code, bytes.to_vec())?;
            Ok(sequence.to_le_bytes().to_vec())
        }
        67 => {
            room(cap, 8)?;
            r.end()?;
            Ok(host.launch_options.ok_or(Error::Unsupported)?.to_vec())
        }
        68 => {
            r.end()?;
            room(cap, 8)?;
            let value = crate::random::value().map_err(|_| Error::Failed)?;
            Ok(value.to_le_bytes().to_vec())
        }
        69 => {
            room(cap, 8)?;
            let grant = r.u64()?;
            let encoded = read_png_asset(host, grant, r.0)?;
            let image = crate::drawing::image::Image::from_png(&encoded).ok_or(Error::Invalid)?;
            let [width, height] = image.size();
            let id = retain_image(host, width as u32, height as u32, image.pixels())?;
            Ok(id.to_le_bytes().to_vec())
        }
        75 => {
            room(cap, 12)?;
            let font = r.u32()?;
            let codepoint = r.u32()?;
            let size = r.u32()?;
            r.end()?;
            if font != 0 {
                return Err(Error::Unsupported);
            }
            let glyph = font::rasterize(codepoint, size).ok_or(Error::Invalid)?;
            room(cap, 12 + glyph.rgba.len())?;
            Ok([
                glyph.width.to_le_bytes().as_slice(),
                &glyph.height.to_le_bytes(),
                &glyph.advance.to_le_bytes(),
                glyph.rgba.as_slice(),
            ]
            .concat())
        }
        158 => {
            room(cap, 24)?;
            let standard = r.u32()?;
            let start = r.u64()?;
            let field = r.u64()?;
            r.end()?;
            let plan = video::frame_plan(standard, start, field).ok_or(Error::Invalid)?;
            Ok([
                plan.field.to_le_bytes().as_slice(),
                &plan.vi_deadline_ns.to_le_bytes(),
                &plan.dac_callbacks.to_le_bytes(),
                &0u32.to_le_bytes(),
            ]
            .concat())
        }
        159 => {
            let width = r.u32()?;
            let height = r.u32()?;
            if width == 0
                || height == 0
                || width > 2048
                || height > 2048
                || !width.is_multiple_of(2)
                || !height.is_multiple_of(2)
            {
                return Err(Error::Invalid);
            }
            let luma = (width as usize)
                .checked_mul(height as usize)
                .ok_or(Error::Invalid)?;
            let chroma = luma / 4;
            let y = r.bytes(luma)?;
            let u = r.bytes(chroma)?;
            let v = r.bytes(chroma)?;
            r.end()?;
            let rgba = video::yuv420_to_rgba(width, height, y, u, v)?;
            room(cap, rgba.len())?;
            Ok(rgba)
        }
        126 => {
            let mask = r.u32()?;
            if mask > 15 {
                return Err(Error::Invalid);
            }
            crate::shaders::nv2a::validate(r.0, std::array::from_fn(|i| mask & (1 << i) != 0))
                .map_err(|e| {
                    if e.unsupported {
                        Error::Unsupported
                    } else {
                        Error::Invalid
                    }
                })?;
            Ok(Vec::new())
        }
        130..=134 | 136..=139 => {
            permitted(host.permissions.audio)?;
            if (137..=139).contains(&op) && host.window.closed {
                return Err(Error::Denied);
            }
            host.streams
                .as_ref()
                .ok_or(Error::Unsupported)?
                .call(op, bytes, cap)
        }
        140..=143 => {
            permitted(host.permissions.microphone)?;
            if host.window.closed {
                return Err(Error::Denied);
            }
            host.backend.capture(op, bytes, cap)
        }
        100..=129 | 135 => {
            if host.window.closed {
                return Err(Error::Denied);
            }
            host.backend.graphics(op, bytes, cap)
        }
        _ => Err(Error::Unsupported),
    }
}

/// Framed private-pipe transport. The launcher supplies the connected streams;
/// this is not a TCP listener, shared global endpoint or game-chosen path.
/// Header: GPH1, operation, request bytes, reply capacity (all LE u32).
/// Reply: signed status, byte count, bytes. Oversize headers terminate before
/// allocation; clean EOF ends the session. Ownership of confinement/launch is
/// deliberately outside this parser.
pub fn serve<B: Backend>(
    host: &mut Session<B>,
    mut input: impl Read,
    mut output: impl Write,
) -> std::io::Result<()> {
    serve_with(&mut input, &mut output, |op, bytes, capacity| {
        host.call(op, bytes, capacity)
    })
}

pub(super) fn serve_with(
    mut input: impl Read,
    mut output: impl Write,
    mut call: impl FnMut(u32, &[u8], u32) -> Result<Vec<u8>>,
) -> std::io::Result<()> {
    loop {
        let mut header = [0; 16];
        match input.read(&mut header[..1]) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
        input.read_exact(&mut header[1..])?;
        let number = |n| u32::from_le_bytes(header[n..n + 4].try_into().unwrap());
        let op = number(4);
        let len = number(8);
        let cap = number(12);
        if &header[..4] != b"GPH1" || len as usize > MAX_MESSAGE || cap as usize > MAX_MESSAGE {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid port service frame",
            ));
        }
        let mut bytes = vec![0; len as usize];
        input.read_exact(&mut bytes)?;
        let (status, reply) = match call(op, &bytes, cap) {
            Ok(v) => (0, v),
            Err(e) => (e as i32, Vec::new()),
        };
        output.write_all(&status.to_le_bytes())?;
        output.write_all(&(reply.len() as u32).to_le_bytes())?;
        output.write_all(&reply)?;
        output.flush()?;
    }
}

pub(super) fn file_operation(op: u32) -> bool {
    matches!(op,1..=9|12..=19|23..=26)
}
fn retain_image<B: Backend>(
    host: &mut Session<B>,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<u64> {
    let len = rgba.len();
    if host.images.len() >= MAX_IMAGES || len > MAX_IMAGE_BYTES.saturating_sub(host.image_bytes) {
        return Err(Error::Busy);
    }
    let id = identifier()?;
    host.images.insert(
        id,
        Arc::new(Image {
            width,
            height,
            rgba: rgba.to_vec(),
        }),
    );
    host.image_bytes += len;
    Ok(id)
}

fn read_png_asset<B: Backend>(host: &Session<B>, grant: u64, name: &[u8]) -> Result<Vec<u8>> {
    let mut files = host.files.lock().map_err(|_| Error::Failed)?;
    if !files.grants().contains(&(grant, 1)) {
        return Err(Error::Denied);
    }
    let handle = files.open(grant, 0, name)?;
    let result = (|| {
        let size = files.size(handle)?;
        if size == 0 || size > crate::drawing::image::MAX_PNG_BYTES as u64 {
            return Err(Error::Invalid);
        }
        let mut contents = Vec::with_capacity(size as usize);
        while contents.len() < size as usize {
            let length = (size as usize - contents.len()).min(65_536);
            let bytes = files.read(handle, contents.len() as u64, length)?;
            if bytes.len() != length {
                return Err(Error::Failed);
            }
            contents.extend(bytes);
        }
        Ok(contents)
    })();
    let closed = files.close(handle);
    let contents = result?;
    closed?;
    Ok(contents)
}

pub(super) fn file_dispatch(
    files: &mut super::filesystem::Files,
    op: u32,
    bytes: &[u8],
    cap: usize,
) -> Result<Vec<u8>> {
    let mut r = Request(bytes);
    match op {
        1 => {
            room(cap, 8)?;
            let root = r.u64()?;
            let mode = r.u32()?;
            Ok(files.open(root, mode, r.0)?.to_le_bytes().to_vec())
        }
        2 => {
            let handle = r.u64()?;
            let offset = r.u64()?;
            let length = r.u32()? as usize;
            r.end()?;
            room(cap, length)?;
            files.read(handle, offset, length)
        }
        3 => {
            let handle = r.u64()?;
            let offset = r.u64()?;
            files.write(handle, offset, r.0)?;
            Ok(Vec::new())
        }
        4 => {
            let handle = r.u64()?;
            r.end()?;
            files.close(handle)?;
            Ok(Vec::new())
        }
        5 => {
            let handle = r.u64()?;
            r.end()?;
            room(cap, 8)?;
            Ok(files.size(handle)?.to_le_bytes().to_vec())
        }
        6 => {
            let root = r.u64()?;
            files.list(root, r.0, cap)
        }
        7 => {
            let root = r.u64()?;
            files.mkdir(root, r.0)?;
            Ok(Vec::new())
        }
        8 => {
            let root = r.u64()?;
            files.remove(root, r.0)?;
            Ok(Vec::new())
        }
        9 => {
            let file = r.u64()?;
            let size = r.u64()?;
            r.end()?;
            files.resize(file, size)?;
            Ok(Vec::new())
        }
        12 => {
            let file = r.u64()?;
            r.end()?;
            files.sync(file)?;
            Ok(Vec::new())
        }
        13 => {
            r.end()?;
            let grants = files.grants();
            room(cap, grants.len() * 16)?;
            let mut out = Vec::new();
            for (id, kind) in grants {
                out.extend(id.to_le_bytes());
                out.extend(kind.to_le_bytes());
                out.extend(0u32.to_le_bytes());
            }
            Ok(out)
        }
        14 => {
            let grant = r.u64()?;
            let flags = r.u32()?;
            let length = r.u32()? as usize;
            if flags > 1 {
                return Err(Error::Invalid);
            }
            let source = r.bytes(length)?;
            files.replace(grant, source, r.0, flags == 1)?;
            Ok(Vec::new())
        }
        15 => {
            let file = r.u64()?;
            r.end()?;
            room(cap, 24)?;
            let (size, seconds, nanos) = files.metadata(file)?;
            Ok([
                size.to_le_bytes().as_slice(),
                &seconds.to_le_bytes(),
                &nanos.to_le_bytes(),
                &0u32.to_le_bytes(),
            ]
            .concat())
        }
        16 => {
            let file = r.u64()?;
            r.end()?;
            // Validate reply space before acquiring a retained handle.
            room(cap, 8)?;
            Ok(files.duplicate(file)?.to_le_bytes().to_vec())
        }
        17 => {
            room(cap, 4)?;
            let grant = r.u64()?;
            Ok(u32::from(files.exists(grant, r.0)?).to_le_bytes().to_vec())
        }
        18 => {
            let grant = r.u64()?;
            files.remove_directory(grant, r.0)?;
            Ok(Vec::new())
        }
        19 => {
            let grant = r.u64()?;
            let start = r.u64()?;
            let length = r.u64()?;
            r.end()?;
            room(cap, 8)?;
            Ok(files.rom_view(grant, start, length)?.to_le_bytes().to_vec())
        }
        23 => {
            let grant = r.u64()?;
            r.end()?;
            room(cap, 16)?;
            let (limit, available) = files.save_volume(grant)?;
            Ok([limit.to_le_bytes(), available.to_le_bytes()].concat())
        }
        24 => {
            let grant = r.u64()?;
            let mode = r.u32()?;
            if mode > 1 {
                return Err(Error::Invalid);
            }
            room(cap, 4)?;
            Ok(files
                .save_directory(grant, r.0, mode == 1)?
                .to_le_bytes()
                .to_vec())
        }
        25 => {
            let handle = r.u64()?;
            r.end()?;
            room(cap, 48)?;
            let (length, times, attributes) = files.save_file_info(handle)?;
            let mut out = Vec::with_capacity(48);
            out.extend(length.to_le_bytes());
            for time in times {
                out.extend(time.to_le_bytes());
            }
            out.extend(attributes.to_le_bytes());
            out.extend(0u32.to_le_bytes());
            Ok(out)
        }
        26 => {
            let handle = r.u64()?;
            let attributes = r.u32()?;
            r.end()?;
            files.save_file_set_attributes(handle, attributes)?;
            Ok(Vec::new())
        }
        _ => Err(Error::Unsupported),
    }
}

pub(super) fn network_dispatch(
    network: &mut crate::network::Service,
    allowed: bool,
    bytes: &[u8],
    cap: usize,
) -> Result<Vec<u8>> {
    let mut r = Request(bytes);
    permitted(allowed)?;
    let op = r.u32()?;
    if op > 17 {
        return Err(Error::Invalid);
    }
    if matches!(op, 0 | 2 | 3 | 4 | 7 | 8 | 10 | 12 | 15 | 16 | 17) {
        room(cap, 4)?;
    }
    if matches!(op, 11 | 13) {
        room(cap, 32)?;
    }
    // The underlying service validates URL/endpoint syntax, budgets,
    // handle ownership, asynchronous status and signed permission.
    let reply = network.call(op as u8, r.0, cap);
    match reply.first() {
        Some(0) => {
            room(cap, reply.len() - 1)?;
            Ok(reply[1..].to_vec())
        }
        Some(2) => Err(Error::Busy),
        Some(3) => Err(Error::Failed),
        _ => Err(Error::Denied),
    }
}
