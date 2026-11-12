# Host audio decoder

This trusted companion decodes compressed music in an import-free Wasm instance.
Current source-built hosts connect it to desktop native/core/component playback
and core/component browser exports. Games use the injected music and recorded
effect functions; they cannot use Cargo dependencies. Maintainer builds embed
the companions, so players need no decoder files, Python or WSL. No decoder
binary or music is checked in.

See [recorded effects and music](../../docs/DEVELOPING_GAMES.md#recorded-effects-on-desktop)
for signed permissions, asset formats, memory shares and game calls. The canonical
Arkanoo recipe uses original recordings. Published app 0.1.17 predates these
source changes. Exact original MP3 codec parity, physical audio output and full
game/platform fidelity remain separate qualification work.

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
starts. Rewind preserves the selected output rate. The companion handles decoding;
the host service supplies asynchronous playback and controls, and game code
chooses its playlist. Declared audio shares reserve decoder, staging and converter
memory inside the existing game ceiling before playback starts.

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
original 44.1 kHz playback pipeline requires the separate compatibility mode below.
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
nine input rates and both output rates. The corresponding mono matrix adds
414 cases, retaining those lengths and adding 2,048-frame boundary edges.
This includes the surprising behavior
that a final non-staged put does not flush retained padding. Tests preserve that
behavior explicitly. `Mix_PlayMusic(1)` is one traversal, as checked by the
original-API capture and its complete reference length.

```sh
python3 tools/check_audio_decoder.py /path/to/original/music --sdl-reference --output-rate 44100 --original-mixer
```

In this mode the corpus reference uses SDL_mixer's actual chunked conversion,
previously verified against `Mix_LoadMUS`/postmix capture. Full native and browser
passes check chunk invariance, retained output rate on rewind, rejected rate
changes and full-track length. All nine original source recordings are stereo.
The original mono Music API also uses 4,096-byte puts: 2,048 S16 mono frames.
Mono input is now supported using a fixed 4 KiB source-put staging buffer and
announced complete puts. The unchanged 2,048-frame conversion ring consumes each
put incrementally with backpressure and waits for its filter lookahead. The
existing hard decoder memory ceiling covers the staging buffer; it is not raised.
New decoder PCM differences remain observable; exact conversion of the same
quantized PCM does not establish bit-identical MP3 codec output. Complete game
playback/campaign and wider platform qualification remain separate work.

Pass `--mono-input /path/to/mono.mp3` to check both original and global conversion
at both output rates in native and Chromium execution. Complete mono PCM hashes,
channel equality, nonzero audio, endings and rewind with varied reads must match.
These checks cannot pass solely on initial silence.

`python3 tools/check_mono_music_schedule.py` verifies the actual Music API's mono
put format/sizes at both output rates using private generated audio.
`python3 tools/check_mono_decoder.py` generates mono MP3 fixtures at all nine
supported source rates and checks both output rates against actual SDL S16
conversion of the same decoded PCM, with varied reads, rewind and the unchanged
6 MiB bound. Add `--worker` to also load each generated mono fixture from a
verified signed package through the asynchronous output-clock worker at 44.1 and
48 kHz. This checks exact SDL-converted output, audio-permission denial, discarded
control transactions, pause/resume, mute with an advancing clock, replay, stop,
and complete shared-reservation cleanup. It preserves the separate complete
nine-track reference corpus and its fixed hashes. On Linux,
`--worker --null-device` also uses private generated three-second mono fixtures
and a private ALSA null configuration to exercise actual CPAL callbacks at both
worker rates. Master mute must still advance playback, unmuted callbacks must
receive nonzero output, midstream pause/resume and stop must work without game
ticks, and all reservations must release. Wrong-rate direct attachment of the
original-rate source is rejected. CI runs this generated suite without original
recordings or a physical sound device. Build the companion at that
ceiling first. The float conversion
matrix and actual Music API put-size trace also run in CI. These tools and
FFmpeg/SDL/Chromium are maintainer dependencies, never player requirements.

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
accessed. This exercises loading through a real signed package independently of
the game music API.

`Factory::open_budgeted` reserves the complete chosen ceiling from a shared
`memory_budget::Budget` before loading. The non-cloneable reservation stays with
the stream until after its Wasm store is destroyed. Failed loads release it;
concurrent reservations cannot overcommit. Tests combine a 10 MiB guest and
6 MiB decoder reservation under one 16 MiB ceiling and reject another decoder
before reading input.

The runtime partitions declared audio shares from the selected game ceiling and
reserves guest memory, decoder and host queues before exposing playback. Stored
entries and resolved shared snapshots are supported; compressed embedded music
is rejected because its decompressor workspace is not accounted. The decoder
ceiling describes linear memory, not total host-process memory or shared JIT
code. Existing game limits are not increased. Browser playback accounts for
decoder input, cache, transfer and converter reservations; browser JIT/GC and
caller-owned asset storage are separate from those reservations. Sequential
browser decoder uploads use at most 64 KiB per transfer, a fixed five-second
aggregate allowance and 100 ms acknowledgement/codec-operation deadlines.

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
The game runtime wires this service through signed audio permission and declared
memory shares. Helpers are bundled in maintainer app builds, and the canonical
port supplies playlist/menu controls. Desktop and browser paths have separate
integration checks; physical device/platform fidelity needs broader verification.

Combined recorded playback uses original-rate music-first mixing. The legacy
generic callback adds
music to already mixed guest PCM, whereas [SDL_mixer mixes music before individual
effect channels](https://github.com/libsdl-org/SDL_mixer/blob/release-2.8.0/src/mixer.c).
Intermediate saturation makes these different: music 30,000 followed by effects
+30,000 and -30,000 yields 2,767 in that order, versus 30,000 if effects are
premixed first. The recorded path therefore mixes individual effect voices after
music before one shared conversion. It rejects premixed guest PCM submission.

The host-only `audio_voices` module implements four first-free effect slots with
shared, immutable, budget-reserved stereo recordings and sequential S16 clipping
after the supplied music frame. It performs no allocation while mixing; pause,
zero-volume advancement, slot exhaustion/reuse and ownership cleanup are tested.
`python3 tools/check_audio_mix.py` independently checks the pinned SDL 2.30.0
oracle (all 65,536 S16 sources, all 129 volumes, seven destination values). The
normal Rust test checks all 59,179,008 results through per-volume full hashes.
The oracle needs only maintainer SDL headers/library and a C compiler; players
need neither. Guest controls reach this trusted host module through the runtime,
and browser exports implement the same recorded-audio contract. The canonical
port uses recorded playback. The separate premixed guest PCM path cannot recover
individual-channel clipping order.

`music::Service::new_original_rate` now supplies rate-tagged 44.1 kHz PCM for
`audio_mix_stream::Playback`. It mixes music followed by the four effect voices
before one shared 48 kHz conversion, driven by the CPAL clock through
`audio::Player::original_source`. The regular music attachment accepts only
48 kHz sources. Combined mode rejects legacy PCM submissions. The converter and
adapter reserve their storage from the caller's shared game budget; clips retain
separate shared recording leases. Injected game helpers select recorded playback
through the declared audio shares; games do not access host objects directly.

The SDL mix oracle also checks complete combined conversion at six volumes with
four unequal stereo clips. Rust requires exact whole-output hashes at three
input/output chunk schedules. `check_music_worker.py --null-device` additionally
requires the actual original-rate callback test: music reaches output, effects
pause/mute and continue with music stopped, and all reservations release without
a game tick. It retains the full nine-track 48 kHz regression. Browser playback
has separate exported-player checks. Physical output and complete original-game
event/stop timing still require qualification.


CPAL playback state is now callback-owned. A fixed atomic 4,800-frame PCM ring
preserves the existing packet/capacity limits; producer controls use a separate
32-entry queue. Busy producer/control locks delay applying commands but do not
silence ready PCM/music. Effect methods acknowledge enqueueing, with callback
voice exhaustion surfaced through `Player::error`. Clear boundaries preserve
later PCM, and pending combined-source detaches cannot consume newly submitted
legacy PCM. Closed consumers reject further producer submissions.

The shared reservation ledger is atomic, so releasing a last recording owner
cannot wait for a budget mutex. The null-device maintainer check explicitly runs
`audio::tests::audio_output_null_device`, which requires all 1,600 queued PCM
frames to reach the actual callback while the producer/control lock stays held.
This is a host-lock regression, not proof of hard real-time allocator/OS behavior
or physical-speaker/platform fidelity. Game event timing requires game-level
checks in addition to this host callback regression.

Transient-output rejection covers host persistence errors. Core/component and
native ticks discard PCM
and drawing when a save/history write fails. The focused core/component regression
first reproduces the old stale PCM and then verifies both failure types after a
successful emitting tick. `python3 tools/check_tick_output_rollback.py` creates a
private synthetic signed package, builds native code, and verifies the same
behavior in the actual native sandbox. This does not establish atomic persistent
saves/history or rollback of already emitted background-engine audio. Recorded
effect/music commands are staged through the signed SDK and published only after
a successful tick and persistence checkpoint; failed ticks discard new controls.

### Embedding in the app

Maintainer app builds set `GAMEPKG_BUNDLED_AUDIO_DECODER` to the compiled decoder
and `GAMEPKG_BUNDLED_AUDIO_CONVERTER` to the converter before building the root
crate. Both are required for recorded-audio browser exports. The root build script copies at most
4 MiB of core Wasm into Cargo's output and embeds those bytes in the executables.
No decoder file, Python interpreter, URL or runtime environment variable is
consulted by `Factory::bundled`. Removing the optional build input writes an
empty asset rather than retaining a previous helper; such source builds report
an explicit unavailable error. Header/size guards run at build time. The factory
then verifies import isolation, bounded memory (at most 6 MiB), and every required
function signature. These remain trusted host dependencies, never guest crates.

For a Rust-only source build on a Unix shell:

```sh
rustup target add wasm32-unknown-unknown
GAMEPKG_AUDIO_MAX_MEMORY_BYTES=6291456 cargo build --locked --release --manifest-path tools/audio-decoder/Cargo.toml --target wasm32-unknown-unknown
cargo build --locked --release --manifest-path tools/audio-converter/Cargo.toml --target wasm32-unknown-unknown
GAMEPKG_BUNDLED_AUDIO_DECODER="$PWD/tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm" \
GAMEPKG_BUNDLED_AUDIO_CONVERTER="$PWD/tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm" \
cargo build --locked --release --features gui --bins
```

On Windows PowerShell, using native Rust without WSL:

```powershell
rustup target add wasm32-unknown-unknown
$env:GAMEPKG_AUDIO_MAX_MEMORY_BYTES = "6291456"
cargo build --locked --release --manifest-path tools/audio-decoder/Cargo.toml --target wasm32-unknown-unknown
cargo build --locked --release --manifest-path tools/audio-converter/Cargo.toml --target wasm32-unknown-unknown
$env:GAMEPKG_BUNDLED_AUDIO_DECODER = (Resolve-Path "tools/audio-decoder/target/wasm32-unknown-unknown/release/gamepkg_host_audio_decoder.wasm").Path
$env:GAMEPKG_BUNDLED_AUDIO_CONVERTER = (Resolve-Path "tools/audio-converter/target/wasm32-unknown-unknown/release/gamepkg_host_audio_converter.wasm").Path
cargo build --locked --release --features gui --bins
```

Hosted maintainer builds use `tools/build_audio_companion.py --github-env` before
the root build. It builds both the decoder and dependency-free converter and sets
`GAMEPKG_BUNDLED_AUDIO_DECODER` and `GAMEPKG_BUNDLED_AUDIO_CONVERTER`.
Recorded-audio web exports embed both runtime sources and integrity-checked
companions; exported players require neither Python nor an external decoder. The packaging source/notices graph now includes the companion's
Wasm dependencies separately from the native host dependencies; its own source
is included without build artifacts. `tests/audio_companion_metadata.py` checks
pinned decoder source/license coverage. The build workflow edits are not evidence
of a completed Windows/Android runtime run.

`tools/check_bundled_audio.py target/release` relocates both executables into a
private directory and checks identical decoder metadata with invalid runtime
paths and no display. `tools/check_music_worker.py <original-music-directory>
--null-device --bundled` runs the nine complete track hashes and actual callback
gates using only embedded bytes, removing decoder path variables before playback.
The standalone build-script test covers malformed/missing/oversized inputs and
resetting an optional input. Embedding supplies the decoder; signed permissions,
per-game service creation, reserved shares and transactional SDK commands govern
its use. Browser exports include the integrity-checked companions and the
canonical port declares recorded effects/music shares.

The shared budget can now partition a fixed child capacity inside the existing
game limit. A child is not another independent allowance. Every child lease
retains the whole parent charge until the final child budget/lease owner drops;
a cache can close while a playing recording keeps its charge. Nested partitions,
concurrent admission, failure cleanup and callback-side clip completion have
regressions. Releases remain atomic, with no reservation mutex on the callback.
The combined real-callback test now plays an effect from a child cache after the
cache object is dropped, and verifies final parent cleanup. Actual 32-bit Linux
checks also exercise full-width counters and cross-thread final ownership.
The same public suites execute in Windows32 and Android32 qualification. See
[platform evidence](../../docs/native-windows-gameplay-validation.md) for their
scope and run identities. Per-game limit carving and signed SDK activation use
these child budgets; executable/userspace checks do not qualify a physical
32-bit OS or Android game-window/audio integration.

### Original-rate effect asset loading

`audio_voices::Clip::from_verified_asset` is a trusted-host loader for canonical
44.1 kHz stereo S16 WAVs. It requires audio permission and a verified scoped
asset. The existing 2 MiB decoded-clip ceiling remains; loading reserves the PCM,
clip ownership and fixed 4,140-byte input staging inside the caller's shared
budget before allocating the sample storage. Bytes stream into the final clip,
with explicit little-endian conversion and no complete encoded duplicate. The
package reader completes its ZIP checksum check before publishing the clip.
Malformed/truncated files, exhausted budgets and corrupt assets release their
charges. Embedded compressed assets fail explicitly pending decompressor-memory
accounting; shared snapshots and stored assets use the bounded reader path.

Maintainers can verify all five original effects without changing port assets:

```sh
python3 tools/check_effect_assets.py /path/to/pinned/original/arkanoo
```

The check validates the original MP3 hashes, uses the independent SDL reference
to generate temporary 44.1 kHz WAVs, signs a temporary package with a synthetic
test key and compares complete playback with every original stereo PCM frame.
It also checks all five recordings fit the unchanged 4 MiB pool and that live
voices retain its parent charge after the cache owner drops. Ordinary tests use
synthetic signed WAVs and require no original assets or SDL decoder. The helper
and native SDL libraries are maintainer dependencies only.

This loader remains trusted host code. Injected guest effect commands reach it
through transactional tick commit and per-game memory shares. Browser playback
implements the same asset/command contract, and the canonical port uses original
recordings. Exact original event timing needs the separate game-level tests.

### Whole-tick effect handoff

`audio::Player::effect_batch` publishes up to 32 host-owned `EffectCommand`s
under one producer lock. Play, volume, pause and stop retain their order. Invalid
volume, overlong batches, insufficient remaining queue space, inactive original
sources and closed devices reject the complete nonempty batch; rejected batches
retain no extra clip references. An empty batch is a no-op while the device is
open. The callback sees a whole accepted batch at its next control boundary, or
keeps rendering its existing state if the producer lock is busy. This uses the
existing 32-command queue and its preallocated storage. Fifth-voice exhaustion
is still an asynchronous playback error rather than enqueue rejection.

The individual effect methods use this same path. Tests cover the exact shared
queue boundary, invalid trailing commands, preserved clips, ordered application,
closed consumers and eight concurrent producers with no batch interleaving. The
private null-device check now batches muted/paused effect startup and resume;
an invalid Stop/volume batch cannot kill the retained voice, which subsequently
produces real callback output after resume. No game ticks advance that playback.

Guest runtime staging publishes through this handoff only after a successful tick
and save/history checkpoint. The matching browser transaction prepares controls
before its checkpoint and applies them after success. These commit rules do not
by themselves establish original game event/stop timing.


## Codec precision provenance

`python3 tools/check_music_codec_precision.py --float-check` uses a private
generated stereo fixture to identify the actual original Music API backend and
compare native/generic MPG123 S16 output, float output quantized with explicit
ties-to-even rounding, and the sandboxed decoder. Pass the original music folder
to run the same checks on the complete local corpus. Only native 44.1/48 kHz
stereo inputs are used so no sample-rate conversion obscures codec arithmetic.
Captured PCM is temporary, and the tool emits metadata only. It requires
maintainer MPG123 development headers, SDL_mixer, FFmpeg and the built companion;
none becomes a player dependency.

On the tested SDL 2.30.0 / SDL_mixer 2.8.0 / MPG123 1.32.5 system, the actual Music
API selects MPG123's AVX path. Direct MPG123 S16 matches its complete PCM exactly,
excluding only confirmed silent callback padding. Across all nine original
tracks (121,627,854 native-rate samples), MPG123 generic versus AVX differs at
89,767 samples by one integer unit. Each path's float output quantizes back to
its own S16 output exactly. The sandboxed minimp3-family decoder differs from
both paths. These observations locate the remaining difference in codec
arithmetic, rather than a missing rounding switch. They do not close exact codec
parity or justify altering existing tolerances, corpus coverage or golden hashes.
The generated provenance probe runs in CI; the original corpus remains private.
## Measured conversion optimization

The shared native/browser resampler now reuses phase weights and input lookups
between stereo channels, preserving each channel's original addition order.
Equal-rate playback copies bounded ring spans without changing queued-history
or backpressure behavior. The filter and memory-state sizes are unchanged.
The updated decoder and output converter use that same source.

The [preserved-baseline benchmark](../audio-resample-bench/README.md) records
2592 exact-output comparisons/5963184 stereo frames on native64, actual ELF32,
actual PE32 under Wine and WebAssembly. Original SDL mono/stereo/flush gates
also pass. On the measured Ryzen5 9600X host, O3/native/thin-LTO conversion
medians improve1.122–1.282x and equal-rate medians1.232–4.536x, with separate
generic-target results and raw min/median/max rows retained. These are component
measurements, not whole-game frame-rate or browser-speed claims. Heavy measured
optimization across every current port and full port/platform fidelity remain
part of the larger project goal.
