# Trusted browser output converter

This maintainer companion reuses GamePKG's fixed-memory adaptation of the SDL
2.30 resampler. It converts already mixed 44.1 kHz stereo S16 input into 48 kHz
stereo S16 output. The browser mixer applies music first, then each effect voice
with sequential integer saturation before calling this single converter.

Build with Rust 1.89 or later and the `wasm32-unknown-unknown` target:

```
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path tools/audio-converter/Cargo.toml
```

It has no Cargo dependencies or host imports. Its Wasm linear memory has a fixed
256 KiB maximum, including a 128 KiB stack reservation; the input/output and
conversion buffers are fixed. Append/render calls accept at most 800 stereo
frames. After initialization, conversion does not allocate. Initialization,
capacity, input/output pointers, append, render and finish use a checked i32 ABI.
The companion and raw memory are never exposed to game source.

Run `python3 tools/check_browser_audio_mixer.py --browser` for the pinned SDL
arithmetic, combined conversion and actual AudioWorklet checks. Python, Node,
the Rust compiler and this source tree are maintainer prerequisites only.

The new companion is not embedded in app releases or activated by browser export
yet. Shared queue/transport accounting and committed game-control integration
remain required before enabling browser recorded audio. Linear-memory and PCM
buffer accounting do not establish a bound on the browser engine's entire heap.

Glue is Apache-2.0; shared resampling code and coefficients retain their original
Zlib/SDL notices in `src/audio_resample.rs` and `src/audio_resample_filter.rs`.
