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
python3 tools/check_sdl_music_reference.py /path/to/original/music --trace
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
Capture setup and shutdown use `Mix_PauseAudio`, which controls SDL_mixer's
actual device. Pausing SDL's separate legacy device does not synchronize these
callbacks and can introduce leading silence; the oracle no longer does that.

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

## Original mixer compatibility

`Factory::open_original_mixer` and the companion's `decoder_set_mixer_rate`
select SDL_mixer 2.8 / SDL 2.30's original block schedule. This is distinct from
the global-phase mode. An actual original-API trace establishes signed 16-bit
stereo input with 4,096-byte puts (1,024 frames). The compatibility converter
restarts phase at those internal boundaries, retains SDL's padding, stages short
last puts and reproduces its flush/truncation behavior. Consumer read sizes do
not alter those boundaries. It uses the existing 2,048-frame ring and reports
input capacity so the decoder observes backpressure without enlarging it.

The optional Linux `--trace` check builds a temporary SDL interposition library
and verifies stream formats and the first 16 put sizes in each original-API
capture. It records bounded metadata only, never PCM, and is not part of the app.

An independent actual `SDL_AudioStream` oracle matches every float sample bit
and complete length across 306 cases: 17 lengths around staging/put boundaries,
nine input rates and both output rates. This includes the surprising behavior
that a final non-staged put does not flush retained padding. Tests preserve that
behavior explicitly. `Mix_PlayMusic(1)` is one traversal, as checked by the
original-API capture and its complete reference length.

```sh
python3 tools/check_audio_decoder.py /path/to/original/music --sdl-reference --output-rate 44100 --original-mixer
```

In this mode the corpus reference uses SDL_mixer's actual chunked conversion,
previously verified against `Mix_LoadMUS`/postmix capture. Full native and browser
passes check chunk invariance, retained output rate on rewind, rejected rate
changes and full-track length. The original mode currently requires stereo
source recordings, as used by all nine tracks. Mono source is rejected because
the original decoder's byte-sized puts imply different frame boundaries; no
claim is made for that unimplemented path. New decoder PCM differences remain
observable. Game playback, controls, scheduling and combined memory accounting
still require integration.

Pass `--mono-input /path/to/mono.mp3` to check the explicit original-mode
rejection alongside complete audible ordinary stereo conversion in native and
Chromium execution. The channel check includes nonzero PCM and the ending, so
it cannot pass solely on the recording's initial silence.

## Loading and shared memory reservations

`Factory::open_reader` writes exact-length trusted input directly into companion
memory using reads of at most 64 KiB. It rejects short input, I/O errors, trailing
bytes and invalid destination ranges without allocating another complete encoded
buffer. `Package::with_asset_reader` supplies a scoped verified-asset reader for
stored/Zstd entries or resolved shared snapshots. It checks paths and sizes
before invoking the consumer, resets snapshot position on each borrow, and
completes the ZIP CRC check after a consumer reads all bytes. A returned resource
is destroyed if that final integrity check fails. The method extracts no files.

The native store accepts a ceiling below the companion's absolute 16 MiB maximum.
The build can also lower the declared maximum, so browsers enforce the same
ceiling rather than checking only after allocation:

```sh
GAMEPKG_AUDIO_MAX_MEMORY_BYTES=6291456 cargo build --locked --release --target wasm32-unknown-unknown --manifest-path tools/audio-decoder/Cargo.toml
python3 tools/check_audio_decoder.py /path/to/original/music --sdl-reference --output-rate 44100 --original-mixer --memory-mib 6 --package-readers
```

The override must be page-aligned and cannot exceed 16 MiB. All nine tracks
complete both passes at enforced 8 MiB and 6 MiB ceilings, including rewind,
malformed input and memory-growth rejection. The largest measured native linear
allocation is 5,373,952 bytes. Browser probes reject growth beyond the selected
declaration. The corpus test loads files directly rather than retaining another
full encoded input vector.

`--package-readers` creates a private signed fixture with a fixed synthetic test
key, verifies it, then loads every track directly through the asset reader. It
also checks that an existing asset is unavailable before verification. Fixture
creation is maintainer work; the archive and copied inputs stay temporary and
are deleted when the test finishes. No user/release key or installed library is
accessed. This exercises loading through a real signed package without enabling
the unfinished game music API.

`Factory::open_budgeted` reserves the complete chosen ceiling from a shared
`memory_budget::Budget` before loading. The non-cloneable reservation stays with
the stream until after its Wasm store is destroyed. Failed loads release it;
concurrent reservations cannot overcommit. Tests combine a 10 MiB guest and
6 MiB decoder reservation under one 16 MiB ceiling and reject another decoder
before reading input.

This provides the loading/reservation mechanism, not completed runtime
accounting. The runtime must reserve guest, decoder and host queues together
before exposing music calls. Archive decompressor workspace and playback buffers
also require accounting; the decoder ceiling describes linear memory, not total
host-process memory or shared JIT code. Asynchronous playback, signed audio
permission integration and browser/desktop SDK calls remain unfinished. Existing
game limits are not increased.
The browser harness still fetches encoded ArrayBuffers; streaming browser input
and its separate buffer accounting remain required before exposing playback.

## Native background playback service

`music::Service` now connects verified assets to a background decoder and a
bounded output-clock queue. It accepts a permission flag from its trusted host
caller, keeps one pending request (latest wins), and provides play, pause,
resume, volume and stop. No game tick is needed to advance playback. The existing
`audio::Player::music_source` consumes this output at source-frame boundaries,
preserves stereo and device phase, clips PCM addition and applies master gain.

Package clones use independent logical cursors over the same already-open file,
with each seek/read protected as one operation and reads bounded to 64 KiB.
They never reopen the cache path. A clone shares private snapshot ownership, so
dropping the parent cannot delete assets still needed by a worker. Tests cover
concurrent stored/Zstd reads, cache-path replacement and last-reader cleanup.

The handoff uses six atomic blocks of up to 800 stereo frames, retaining the
existing 4,800-frame/100 ms queue bound. It uses safe Rust only. Output reads
never acquire the producer/control mutex; a regression holds that mutex while
requiring queued PCM to remain readable. Generation tags discard replaced tracks
without consuming a newer track's data. One guarded consumer owns the output
clock; cloned handles are not independent playback copies. Sticky thread-unpark
tokens prevent missed capacity/control wakeups. Pause freezes consumption, while
zero volume advances silently. End is reported after queued samples are consumed.

The worker reserves 64 KiB for audio staging/queue data and the chosen decoder
ceiling separately. Tests reserve 9.9375 MiB for the guest, 6 MiB for the decoder
and 64 KiB for staging under a 16 MiB total. Decoder errors, insufficient memory,
replacement bursts and shutdown release reservations in ownership order. Output
handles retain the queue's reservation until the last handle drops. These are
managed-data reservations, not a total host-process/JIT/metadata memory limit.

Decoder execution now checks a monotonic 100 ms deadline at epoch wakeups,
without fuel. A delayed-first-epoch regression verifies interruption at that
first expired wakeup, rather than waiting another twenty nominal timer ticks.
Trusted file loading resets the execution deadline before decoding.

```sh
python3 tools/check_music_worker.py /path/to/original/music --null-device
```

The pinned nine-track corpus uses signed temporary fixtures, simultaneous drawing
asset reads and no game ticks. Complete output lengths match original SDL_mixer
at 44.1 kHz followed by whole-buffer SDL conversion to 48 kHz. Full PCM regression
hashes and input hashes are required; remaining one/two-unit SDL decoder
differences are reported separately. The private ALSA null device additionally
verifies actual CPAL callback consumption, muted-master progress, nonzero output,
pause/resume and cleanup while the main thread is idle. This is not physical
speaker or other-platform verification. Users need none of these maintainer tools.

The service currently supports stored embedded assets and resolved raw shared
snapshots. It explicitly rejects compressed embedded music until decompressor
workspace can be reserved. The generic archive reader still supports Zstd.
Game runtime/SDK wiring, helper bundling, playlist/menu controls, browser playback
and streaming input remain unfinished. Existing PCM device-rate conversion also
needs broader fidelity verification.

Combined original music/effect mixing remains open. The generic callback adds
music to already mixed guest PCM, whereas [SDL_mixer mixes music before individual
effect channels](https://github.com/libsdl-org/SDL_mixer/blob/release-2.8.0/src/mixer.c).
Intermediate saturation makes these different: music 30,000 followed by effects
+30,000 and -30,000 yields 2,767 in that order, versus 30,000 if effects are
premixed first. Faithful combined playback needs the original per-voice mixing
and independent effect timing; the current ports are not migrated to this service.

The host-only `audio_voices` module implements four first-free effect slots with
shared, immutable, budget-reserved stereo recordings and sequential S16 clipping
after the supplied music frame. It performs no allocation while mixing; pause,
zero-volume advancement, slot exhaustion/reuse and ownership cleanup are tested.
`python3 tools/check_audio_mix.py` independently checks the pinned SDL 2.30.0
oracle (all 65,536 S16 sources, all 129 volumes, seven destination values). The
normal Rust test checks all 59,179,008 results through per-volume full hashes.
The oracle needs only maintainer SDL headers/library and a C compiler; players
need neither. The module is still host-only: effect-command SDK integration,
original-rate combined mixing before resampling, browser playback and actual
port migration remain unfinished. The current premixed guest PCM path cannot
recover individual-channel clipping order.

`music::Service::new_original_rate` now supplies rate-tagged 44.1 kHz PCM for
`audio_mix_stream::Playback`. It mixes music followed by the four effect voices
before one shared 48 kHz conversion, driven by the CPAL clock through
`audio::Player::original_source`. The regular music attachment accepts only
48 kHz sources. Combined mode rejects legacy PCM submissions. The converter and
adapter reserve their storage from the caller's shared game budget; clips retain
separate shared recording leases. Game SDKs still expose neither path.

The SDL mix oracle also checks complete combined conversion at six volumes with
four unequal stereo clips. Rust requires exact whole-output hashes at three
input/output chunk schedules. `check_music_worker.py --null-device` additionally
requires the actual original-rate callback test: music reaches output, effects
pause/mute and continue with music stopped, and all reservations release without
a game tick. It retains the full nine-track 48 kHz regression. Physical output,
browser integration, original block/event/stop timing and nonblocking host-control
handoff remain open; the CPAL buffer lock can still cause contention silence.
