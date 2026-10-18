# Host audio decoder prototype

This trusted companion decodes compressed music in an import-free Wasm instance.
It is not yet connected to game playback or exposed through the game SDK. Games
still cannot use Cargo dependencies. No decoder binary or music is checked in.

The native host and browser exercise the same scalar decoder. The companion
pins nanomp3 and nanomp3-core 0.2.0 (MIT OR Apache-2.0); SIMD and Layer I/II support
are disabled. It accepts at most 8 MiB of encoded input, declares a 16 MiB memory
maximum, returns at most 1,600 sample values per read, and bounds encoded reads
to 512 KiB per operation. The native wrapper additionally enforces memory and
epoch deadlines without instruction fuel. All input is supplied as bytes; the
companion has no filesystem, networking or other imports.

Output is signed 16-bit PCM at the recording's original sample rate and channel
count. Final float-to-integer conversion uses nearest-even rounding. The host
can also request stereo output at 44.1 or 48 kHz before reading. This uses the
shared fixed-memory SDL-filter converter, and rejects rate changes after reading
starts. Rewind preserves the selected output rate. It does not yet provide a
playlist, music controls, or asynchronous playback.
Integration must account for decoder memory alongside game memory.

## Maintainer verification

Build with Rust's wasm32-unknown-unknown target installed:

```sh
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path tools/audio-decoder/Cargo.toml
cargo test --release --test audio_decoder
python3 tools/check_audio_decoder.py /path/to/original/music --sdl-reference
python3 tools/check_sdl_music_reference.py /path/to/original/music
python3 tools/check_audio_decoder.py /path/to/original/music --sdl-reference --output-rate 48000
```

The final command uses maintainer-only Chromium, SDL_mixer, a C compiler and
ffprobe. Users do not need Python. Temporary recordings and reference output
stay local. The corpus tests stream every track twice with different chunk sizes,
verify rewind, malformed-input handling, memory bounds, complete native PCM
hashes, and browser full-track fingerprints plus exact audible sample windows.
SDL reference lengths are required to match and every sample is compared.

Current evidence covers nine original tracks. Native and Chromium fingerprints
and audible windows agree. All nine SDL lengths agree, but some sample values
differ by one integer unit. The comparison reports these differences; it does
not assert or establish bit-exact SDL parity. Changing integer rounding alone
did not eliminate the differences. Codec arithmetic and the eventual resampling
path still need investigation before claiming original music fidelity.

The separate SDL reference check exercises `Mix_LoadMUS`/`Mix_PlayMusic(1)` and
postmix capture through a disk audio driver with no real-time delay. It requires
every compared sample to match `Mix_LoadWAV` at both the source rate and original
44.1 kHz mixer rate. Only the final callback's zero padding is excluded. Capture
stops after that callback, avoiding timing-dependent extra silence. This verifies
the codec oracle against the original music API; it does not make the new
decoder bit-exact. The disk driver and postmix capture write private temporary
files which the script removes on exit.

## Streaming conversion

`src/audio_resample.rs` adapts SDL 2.30's bandlimited float filter to a fixed
2,048-frame stereo ring. SDL's coefficients and license notice are retained in
`src/audio_resample_filter.rs`. Global integer phase makes conversion independent
of append/read sizes. Reads and appends have an 800-frame bound, retain edge
history and right-side lookahead, and use zero padding only after explicit EOF.
Input backpressure rejects before changing the queue. The helper compiles this
same source for both native Wasmtime and Chromium execution; no dependency is
added to games.

The independent float oracle checks every sample bit across all nine input
rates and both output rates, including unequal stereo channels and edge
impulses. To run it without an audio device:

```sh
cc tools/audio-decoder/resample_reference.c -o /tmp/gamepkg-resample-reference $(pkg-config --cflags --libs sdl2)
GAMEPKG_SDL_RESAMPLE_REFERENCE=/tmp/gamepkg-resample-reference cargo test --release --test audio_resampler -- --ignored
```

With `--output-rate`, the corpus comparison decodes SDL at the source rate then
uses one whole-buffer `SDL_ConvertAudio` conversion. This isolates the converter
and enforces its complete output length. It is deliberately distinct from
`SDL_AudioStream`, whose per-buffer phase restart/truncation changes full-track
duration. For example, the ambient track at 48 kHz has 17,763,784 sample values
with global phase versus 17,753,782 from SDL_mixer's chunked path. Matching the
original 44.1 kHz playback pipeline and its asynchronous scheduling remains open.
The float kernel matches SDL exactly; decoder integer differences can propagate
to two-unit differences after filtering. Those are reported, not accepted as
bit-exact parity.
