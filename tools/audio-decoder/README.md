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
count. Final float-to-integer conversion uses nearest-even rounding. This does
not yet provide resampling, a playlist, music controls, or asynchronous playback.
Integration must account for decoder memory alongside game memory.

## Maintainer verification

Build with Rust's wasm32-unknown-unknown target installed:

```sh
cargo build --locked --release --target wasm32-unknown-unknown --manifest-path tools/audio-decoder/Cargo.toml
cargo test --release --test audio_decoder
python3 tools/check_audio_decoder.py /path/to/original/music --sdl-reference
python3 tools/check_sdl_music_reference.py /path/to/original/music
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
