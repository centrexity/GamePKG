//! Trusted fixed-memory output conversion; never a game dependency.
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
compile_error!("build for wasm32-unknown-unknown");
#[path = "../../../src/audio_resample.rs"]
pub mod resample;
use std::sync::Mutex;
const FRAMES: usize = 800;
struct Context {
    converter: resample::Resampler,
    input: [i16; FRAMES * 2],
    output: [i16; FRAMES * 2],
    scratch: [[f32; 2]; FRAMES],
    finished: bool,
}
static CONTEXT: Mutex<Option<Context>> = Mutex::new(None);
#[no_mangle]
pub extern "C" fn converter_init() -> i32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return -1;
    };
    let Ok(converter) = resample::Resampler::new(44100, 48000) else {
        return -1;
    };
    *context = Some(Context {
        converter,
        input: [0; FRAMES * 2],
        output: [0; FRAMES * 2],
        scratch: [[0.0; 2]; FRAMES],
        finished: false,
    });
    0
}
#[no_mangle]
pub extern "C" fn converter_input() -> u32 {
    CONTEXT
        .lock()
        .ok()
        .and_then(|mut c| c.as_mut().map(|c| c.input.as_mut_ptr() as usize as u32))
        .unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn converter_output() -> u32 {
    CONTEXT
        .lock()
        .ok()
        .and_then(|mut c| c.as_mut().map(|c| c.output.as_mut_ptr() as usize as u32))
        .unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn converter_capacity() -> u32 {
    CONTEXT
        .lock()
        .ok()
        .and_then(|c| {
            c.as_ref().map(|c| {
                if c.finished {
                    0
                } else {
                    c.converter.input_capacity() as u32
                }
            })
        })
        .unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn converter_append(frames: u32) -> i32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return -1;
    };
    let Some(context) = context.as_mut() else {
        return -1;
    };
    let frames = frames as usize;
    if context.finished || frames > FRAMES || frames > context.converter.input_capacity() {
        return -1;
    }
    for (frame, samples) in context.scratch[..frames]
        .iter_mut()
        .zip(context.input[..frames * 2].as_chunks::<2>().0)
    {
        *frame = samples.map(|sample| f32::from(sample) / 32768.0);
    }
    if context
        .converter
        .append(&context.scratch[..frames])
        .is_err()
    {
        -1
    } else {
        0
    }
}
#[no_mangle]
pub extern "C" fn converter_render(frames: u32) -> i32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return -1;
    };
    let Some(context) = context.as_mut() else {
        return -1;
    };
    let frames = frames as usize;
    if frames > FRAMES {
        return -1;
    }
    let Ok(count) = context.converter.render(&mut context.scratch[..frames]) else {
        return -1;
    };
    for (output, input) in context.output[..count * 2]
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .zip(&context.scratch[..count])
    {
        *output = input.map(|sample| {
            (sample * 32768.0)
                .round_ties_even()
                .clamp(-32768.0, 32767.0) as i16
        });
    }
    count as i32
}
#[no_mangle]
pub extern "C" fn converter_finish() -> i32 {
    let Ok(mut context) = CONTEXT.lock() else {
        return -1;
    };
    let Some(context) = context.as_mut() else {
        return -1;
    };
    context.finished = true;
    context.converter.finish();
    0
}
