// App-supplied adapters for dependency-free 2D tick games.
struct TickFrame {
    clear: u32,
    transform: [f32; 3],
    last_tick_us: u64,
    last_tick_counter: u64,
    draws: Vec<TickDraw>,
    clip: Option<[f32; 4]>,
    events: Vec<InputEvent>,
    pointer: [f32; 2],
    pointer_buttons: u32,
    keyboard: [bool; 512],
    touch: Option<(u64, u64)>,
    window_size: [u32; 2],
}

static TICK_FRAME: GameState<TickFrame> = GameState::new(TickFrame {
    clear: 0x000000ff,
    transform: [0.0, 0.0, 1.0],
    last_tick_us: 0,
    last_tick_counter: 0,
    draws: Vec::new(),
    clip: None,
    events: Vec::new(),
    pointer: [0.0; 2],
    pointer_buttons: 0,
    keyboard: [false; 512],
    touch: None,
    window_size: [640, 480],
});

struct TickDraw {
    image: u64,
    solid: bool,
    rect: [f32; 4],
    linear: bool,
    clip: Option<[f32; 4]>,
    circle: bool,
    straight: bool,
}

fn tick_window_metrics() -> std::result::Result<([u32; 2], u32), i32> {
    let mut reply = [0; 12];
    if call(11, &[], &mut reply)? != 12 {
        return Err(-3);
    }
    Ok(([u32_at(&reply, 0), u32_at(&reply, 4)], u32_at(&reply, 8)))
}

pub(super) fn __gamepkg_tick_window() -> std::result::Result<Option<[u32; 2]>, i32> {
    let (size, flags) = tick_window_metrics()?;
    if flags & 4 != 0 {
        return Ok(None);
    }
    let clock = local_performance_sample()?;
    let seconds = clock.counter / clock.frequency;
    let fraction = (clock.counter % clock.frequency) as f64 / clock.frequency as f64;
    let now_us = seconds
        .saturating_mul(1_000_000)
        .saturating_add((fraction * 1_000_000.0) as u64);
    let scale = (size[0] as f32 / 640.0).min(size[1] as f32 / 480.0);
    let mut frame = TICK_FRAME.try_lock().ok_or(-3)?;
    frame.window_size = size;
    frame.last_tick_us = now_us;
    frame.last_tick_counter = clock.counter;
    frame.transform = [
        (size[0] as f32 - 640.0 * scale) / 2.0,
        (size[1] as f32 - 480.0 * scale) / 2.0,
        scale,
    ];
    Ok(Some(size))
}

pub fn frame_time_us() -> u64 {
    TICK_FRAME.try_lock().map_or(0, |frame| frame.last_tick_us)
}

pub fn window_size() -> Option<[u32; 2]> {
    let (size, flags) = tick_window_metrics().ok()?;
    if flags & 4 != 0 || size.contains(&0) {
        return None;
    }
    Some(size)
}

pub fn window_fullscreen() -> Option<bool> {
    let (_, flags) = tick_window_metrics().ok()?;
    (flags & 4 == 0).then_some(flags & 1 != 0)
}

pub fn audio_submit_stereo(samples: &[i16]) -> bool {
    samples.len() <= 1600 && audio(samples, 2).is_ok()
}

#[derive(Clone, Copy)]
pub struct InputEvent {
    pub kind: u32,
    pub x: f32,
    pub y: f32,
    pub code: u32,
}

pub fn input_events() -> Option<Vec<InputEvent>> {
    Some(std::mem::take(&mut TICK_FRAME.try_lock()?.events))
}

fn tick_input_event(frame: &mut TickFrame, e: NativeInputEvent) {
    let [left, top, scale] = frame.transform;
    let pointer = |x, y| [(x - left) / scale, (y - top) / scale];
    let mut push =
        |kind: u32, [x, y]: [f32; 2], code: u32| frame.events.push(InputEvent { kind, x, y, code });
    match e.kind {
        0 => {
            frame.pointer_buttons = 0;
            frame.touch = None;
            push(0, [0.0; 2], 0);
        }
        1 | 2 => {
            push(e.kind + 4, [0.0; 2], e.code);
            let code = match e.code {
                4..=29 => b'a' as u32 + e.code - 4,
                30..=38 => b'1' as u32 + e.code - 30,
                39 => b'0' as u32,
                40 => 13,
                41 => 27,
                42 => 8,
                43 => 9,
                44 => 32,
                76 => 127,
                58..=69 | 73..=75 | 77..=82 => (1 << 30) | e.code,
                _ => 0,
            };
            if code != 0 {
                push(e.kind + 8, [0.0; 2], code);
            }
        }
        3 => {
            frame.pointer = pointer(e.x, e.y);
            push(1, frame.pointer, frame.pointer_buttons);
        }
        5 | 6 => {
            let bit = 1 << e.code.saturating_sub(1).min(4);
            if e.kind == 5 {
                frame.pointer_buttons |= bit;
            } else {
                frame.pointer_buttons &= !bit;
            }
            let code = match e.code {
                2 => 3,
                3 => 2,
                code => code,
            };
            push(e.kind - 3, frame.pointer, code);
        }
        7 => push(4, [e.x, e.y], 0),
        8..=10 => {
            let contact = (e.device, e.contact);
            if e.kind == 8 && frame.touch.is_none() {
                frame.touch = Some(contact);
            }
            if frame.touch == Some(contact) {
                let [w, h] = frame.window_size;
                let xy = pointer(e.x * w as f32, e.y * h as f32);
                if e.kind == 10 {
                    frame.touch = None;
                }
                let kind = match e.kind {
                    8 => 2,
                    9 => 1,
                    _ => 3,
                };
                push(kind, xy, 1);
            }
        }
        17 => push(7, [0.0; 2], e.code),
        _ => {}
    }
}

fn tick_key_button(code: u32) -> u32 {
    match code {
        80 | 4 => 1,
        79 | 7 => 2,
        82 | 26 => 4,
        81 | 22 => 8,
        29 => 16,
        44 | 27 => 32,
        40 => 64,
        43 => 128,
        6 => 65536,
        19 => 131072,
        20 => 262144,
        8 => 524288,
        _ => 0,
    }
}

fn tick_held(buttons: u32) -> u32 {
    ((buttons & 0xff) << 8) | ((buttons & 0x000f0000) << 4)
}

pub(super) fn __gamepkg_tick_input(previous_pad: &mut u32) -> u32 {
    let mut buttons = 0;
    if let (Ok(events), Some(mut frame)) = (native_input_events(), TICK_FRAME.try_lock()) {
        frame.events.clear();
        for e in events {
            tick_input_event(&mut frame, e);
            if e.kind == 0 {
                frame.keyboard.fill(false);
                buttons = 0;
            }
            if matches!(e.kind, 1 | 2) {
                if let Some(key) = frame.keyboard.get_mut(e.code as usize) {
                    *key = e.kind == 1;
                }
                if e.kind == 1 {
                    buttons |= tick_key_button(e.code);
                }
            }
        }
        let held = frame
            .keyboard
            .iter()
            .enumerate()
            .fold(0, |held, (code, down)| {
                held | if *down {
                    tick_key_button(code as u32)
                } else {
                    0
                }
            });
        buttons |= tick_held(held);
    }
    let pad = gamepad(0).ok();
    let logical_pad = pad.map_or(0, |pad| {
        let b = pad.buttons;
        let left = b & 4 != 0 || pad.left_x < -10500;
        let right = b & 8 != 0 || pad.left_x > 10500;
        let up = b & 1 != 0 || pad.left_y > 10500;
        let down = b & 2 != 0 || pad.left_y < -10500;
        u32::from(left)
            | (u32::from(right) << 1)
            | (u32::from(up) << 2)
            | (u32::from(down) << 3)
            | ((b >> 9) & (16 | 32))
            | ((b & 0x30) << 2)
            | ((b & (1 << 12)) << 4)
            | ((b & (1 << 15)) << 2)
            | ((b & 0x300) << 10)
    });
    let pressed = logical_pad & !*previous_pad;
    *previous_pad = logical_pad;
    buttons | pressed | tick_held(logical_pad)
}

pub(super) fn __gamepkg_tick_present() -> std::result::Result<(), i32> {
    let (clear, draws) = {
        let mut frame = TICK_FRAME.try_lock().ok_or(-3)?;
        frame.clip = None;
        (frame.clear, std::mem::take(&mut frame.draws))
    };
    let result = tick_submit_frame(clear, &draws);
    let mut released = Ok(());
    for draw in draws {
        if !draw.solid {
            released = released.and(image_release(draw.image));
        }
    }
    result.and(released)
}

fn tick_submit_frame(clear: u32, draws: &[TickDraw]) -> std::result::Result<(), i32> {
    let mut request = clear.to_le_bytes().to_vec();
    request.extend((draws.len() as u32).to_le_bytes());
    for draw in draws {
        request.extend(draw.image.to_le_bytes());
        for value in draw.rect {
            request.extend(value.to_le_bytes());
        }
        request.extend(u32::from(draw.linear).to_le_bytes());
        request.extend(u32::MAX.to_le_bytes());
        let flags = u32::from(draw.clip.is_some())
            | (u32::from(draw.circle) << 1)
            | (u32::from(draw.straight) << 2)
            | (u32::from(draw.solid) << 3);
        request.extend(flags.to_le_bytes());
        for value in draw.clip.unwrap_or([0.0; 4]) {
            request.extend(value.to_le_bytes());
        }
    }
    for _ in 0..128 {
        match call(70, &request, &mut []).map(|_| ()) {
            Err(-2) => {
                let clock = local_clock_state()?;
                let deadline = clock
                    .sample
                    .counter
                    .saturating_add(clock.sample.frequency.div_ceil(1000));
                local_clock_wait(deadline, clock.sample.frequency, clock.generation)?;
            }
            result => return result,
        }
    }
    Err(-2)
}

pub(super) fn __gamepkg_tick_wait() -> std::result::Result<(), i32> {
    let mut clock = local_clock_state()?;
    let started = TICK_FRAME.try_lock().ok_or(-3)?.last_tick_counter;
    let deadline = started.saturating_add(clock.sample.frequency.div_ceil(60));
    // Provider wakes are useful to waiters, but must not accelerate a fixed tick.
    while clock.sample.counter < deadline {
        local_clock_wait(deadline, clock.sample.frequency, clock.generation)?;
        clock = local_clock_state()?;
    }
    Ok(())
}

pub fn draw_clear(rgba: u32) -> bool {
    let Some(mut frame) = TICK_FRAME.try_lock() else {
        return false;
    };
    for draw in frame.draws.drain(..) {
        if !draw.solid && image_release(draw.image).is_err() {
            return false;
        }
    }
    frame.clear = rgba;
    true
}

pub fn draw_rect(x: f32, y: f32, w: f32, h: f32, rgba: u32) -> bool {
    tick_draw_solid([x, y, w, h], rgba, false)
}

pub fn draw_image(x: f32, y: f32, w: f32, h: f32, pw: u32, ph: u32, pixels: &[u8]) -> bool {
    tick_draw_image([x, y, w, h], [pw, ph], pixels, false)
}

pub fn draw_image_linear(x: f32, y: f32, w: f32, h: f32, pw: u32, ph: u32, pixels: &[u8]) -> bool {
    tick_draw_image([x, y, w, h], [pw, ph], pixels, true)
}

fn tick_draw_image(
    [x, y, w, h]: [f32; 4],
    [pw, ph]: [u32; 2],
    pixels: &[u8],
    linear: bool,
) -> bool {
    if pw > 512
        || ph > 512
        || ![x, y, w, h].iter().all(|value| value.is_finite())
        || w <= 0.0
        || h <= 0.0
    {
        return false;
    }
    let Some(mut frame) = TICK_FRAME.try_lock() else {
        return false;
    };
    if frame.draws.len() >= 4096 {
        return false;
    }
    let [left, top, scale] = frame.transform;
    let rect = [left + x * scale, top + y * scale, w * scale, h * scale];
    let Ok(image) = image_create(pw, ph, pixels) else {
        return false;
    };
    let clip = frame.clip;
    frame.draws.push(TickDraw {
        image,
        solid: false,
        rect,
        linear,
        clip,
        circle: false,
        straight: false,
    });
    true
}

fn tick_asset_grant() -> Option<u64> {
    grants()
        .ok()?
        .into_iter()
        .find(|grant| grant.kind == 1)
        .map(|grant| grant.id)
}

pub fn asset_read(name: &str, offset: u32, out: &mut [u8]) -> Option<u32> {
    if name.is_empty() || name.len() > 1024 || out.len() > 65_536 {
        return None;
    }
    let file = file_open(tick_asset_grant()?, false, name).ok()?;
    let len = scoped_file_read(file, u64::from(offset), out).ok();
    scoped_file_close(file).ok()?;
    u32::try_from(len?).ok()
}

pub fn draw_asset_image(name: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    tick_draw_asset(name, [x, y, w, h], false, false)
}

pub fn draw_asset_image_linear(name: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    tick_draw_asset(name, [x, y, w, h], true, false)
}

pub fn draw_asset_image_linear_straight(name: &str, x: f32, y: f32, w: f32, h: f32) -> bool {
    tick_draw_asset(name, [x, y, w, h], true, true)
}

pub fn draw_clip(x: f32, y: f32, w: f32, h: f32) -> bool {
    let Some(mut frame) = TICK_FRAME.try_lock() else {
        return false;
    };
    if ![x, y, w, h].iter().all(|v| v.is_finite()) || w < 0.0 || h < 0.0 {
        return false;
    }
    let [left, top, scale] = frame.transform;
    frame.clip = Some([left + x * scale, top + y * scale, w * scale, h * scale]);
    true
}

pub fn draw_clip_reset() -> bool {
    let Some(mut frame) = TICK_FRAME.try_lock() else {
        return false;
    };
    frame.clip = None;
    true
}

pub fn draw_circle(x: f32, y: f32, radius: f32, rgba: u32) -> bool {
    let diameter = radius * 2.0;
    tick_draw_solid([x - radius, y - radius, diameter, diameter], rgba, true)
}

fn tick_draw_solid([x, y, w, h]: [f32; 4], rgba: u32, circle: bool) -> bool {
    if ![x, y, w, h].iter().all(|value| value.is_finite()) || w <= 0.0 || h <= 0.0 {
        return false;
    }
    let Some(mut frame) = TICK_FRAME.try_lock() else {
        return false;
    };
    if frame.draws.len() >= 4096 {
        return false;
    }
    let [left, top, scale] = frame.transform;
    let clip = frame.clip;
    frame.draws.push(TickDraw {
        image: u64::from(rgba),
        solid: true,
        rect: [left + x * scale, top + y * scale, w * scale, h * scale],
        linear: false,
        clip,
        circle,
        straight: false,
    });
    true
}

fn tick_draw_asset(name: &str, [x, y, w, h]: [f32; 4], linear: bool, straight: bool) -> bool {
    if name.is_empty()
        || name.len() > 1024
        || ![x, y, w, h].iter().all(|value| value.is_finite())
        || w <= 0.0
        || h <= 0.0
    {
        return false;
    }
    let Some(mut frame) = TICK_FRAME.try_lock() else {
        return false;
    };
    if frame.draws.len() >= 4096 {
        return false;
    }
    let Some(grant) = tick_asset_grant() else {
        return false;
    };
    let mut request = grant.to_le_bytes().to_vec();
    request.extend(name.as_bytes());
    let mut reply = [0; 8];
    if call(69, &request, &mut reply) != Ok(8) {
        return false;
    }
    let [left, top, scale] = frame.transform;
    let clip = frame.clip;
    frame.draws.push(TickDraw {
        image: u64::from_le_bytes(reply),
        solid: false,
        rect: [left + x * scale, top + y * scale, w * scale, h * scale],
        linear,
        clip,
        circle: false,
        straight,
    });
    true
}

fn tick_save_grant() -> Option<u64> {
    grants()
        .ok()?
        .into_iter()
        .find(|grant| grant.kind == 2)
        .map(|grant| grant.id)
}

pub fn save_size() -> Option<u32> {
    let grant = tick_save_grant()?;
    if !file_exists(grant, "tick-save").ok()? {
        return Some(0);
    }
    let file = file_open(grant, false, "tick-save").ok()?;
    let size = file_size(file)
        .ok()
        .and_then(|size| u32::try_from(size).ok());
    scoped_file_close(file).ok()?;
    size
}

pub fn save_read(offset: u32, out: &mut [u8]) -> Option<u32> {
    let file = file_open(tick_save_grant()?, false, "tick-save").ok()?;
    let size = scoped_file_read(file, u64::from(offset), out)
        .ok()
        .and_then(|size| u32::try_from(size).ok());
    scoped_file_close(file).ok()?;
    size
}

pub fn save_write(bytes: &[u8]) -> bool {
    if bytes.len() > 1024 * 1024 {
        return false;
    }
    let Some(grant) = tick_save_grant() else {
        return false;
    };
    const TEMP: &str = "tick-save.pending";
    if file_exists(grant, TEMP) == Ok(true) && file_remove(grant, TEMP).is_err() {
        return false;
    }
    let Ok(file) = file_create_new(grant, TEMP) else {
        return false;
    };
    let written = file_write(file, 0, bytes);
    let closed = scoped_file_close(file);
    written
        .and(closed)
        .and_then(|()| file_replace(grant, TEMP, "tick-save", true))
        .is_ok()
}

// Match the tick API's ordered chunk contract. Publish only the complete
// stream through the same capability-relative atomic replacement as save_write.
static TICK_SAVE_STREAM: GameState<Option<(u32, Vec<u8>)>> = GameState::new(None);

pub fn save_chunk(offset: u32, total: u32, bytes: &[u8]) -> bool {
    let Some(mut pending) = TICK_SAVE_STREAM.try_lock() else {
        return false;
    };
    if total > 1024 * 1024
        || bytes.len() > 65_536
        || u64::from(offset) + bytes.len() as u64 > u64::from(total)
    {
        *pending = None;
        return false;
    }
    if offset == 0 {
        *pending = Some((total, Vec::new()));
    }
    let Some((expected, staged)) = pending.as_mut() else {
        return false;
    };
    if *expected != total || staged.len() != offset as usize {
        *pending = None;
        return false;
    }
    staged.extend_from_slice(bytes);
    if staged.len() != total as usize {
        return true;
    }
    let Some((_, completed)) = pending.take() else {
        return false;
    };
    save_write(&completed)
}

pub fn random_u64() -> u64 {
    let mut reply = [0; 8];
    if call(68, &[], &mut reply) != Ok(8) {
        panic!("GamePKG randomness is unavailable");
    }
    u64::from_le_bytes(reply)
}

fn tick_words(operation: u32, values: &[u32], payload: &[u8]) -> Option<u32> {
    let mut request = Vec::with_capacity(values.len() * 4 + payload.len());
    for value in values {
        request.extend(value.to_le_bytes());
    }
    request.extend_from_slice(payload);
    let mut out = [0; 4];
    (call(operation, &request, &mut out).ok()? == 4).then_some(u32::from_le_bytes(out))
}

pub fn window_control(operation: u32, value: u32) -> bool {
    tick_words(71, &[operation, value], &[]) == Some(1)
}

pub fn window_preview_size(w: u32, h: u32) -> bool {
    w <= 65535 && h <= 65535 && window_control(5, w | (h << 16))
}

pub fn window_revert_size() -> bool {
    window_control(6, 0)
}
pub fn window_keep_size() -> bool {
    window_control(7, 0)
}
pub fn window_preview_active() -> bool {
    window_control(8, 0)
}
pub fn achievement_unlock(index: u32) -> bool {
    tick_words(73, &[index], &[]) == Some(1)
}

fn __gamepkg_effect_call(operation: u32, name: &str, value: u32) -> bool {
    tick_words(72, &[operation, value], name.as_bytes()) == Some(1)
}

fn __gamepkg_file_call(operation: u32, bytes: &[u8], out: &mut [u8]) -> Option<u32> {
    let request = [operation.to_le_bytes().as_slice(), bytes].concat();
    u32::try_from(call(74, &request, out).ok()?).ok()
}

pub fn text_from_utf8_owned(bytes: Vec<u8>) -> std::result::Result<String, Vec<u8>> {
    if text_from_utf8(&bytes).is_err() {
        return Err(bytes);
    }
    // The trusted validator inspected these exact, privately owned bytes.
    Ok(unsafe { String::from_utf8_unchecked(bytes) })
}
