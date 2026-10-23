# Exact-output audio optimization benchmark

This maintainer tool compares the previous shared audio resampler with the
optimized implementation. It uses synthetic signals, no audio device, no game
assets, no dependencies beyond Rust/Node, and no player Python requirement.
It measures the conversion component, not game frame rate or complete-game
performance. Engine-specific profiling and heavy measured optimization across
every current port remain required.

The production change shares filter weights and ring lookups between stereo
channels while retaining each channel's original left/right-wing addition
order. It performs no reassociation, fused multiply/add substitution, coefficient
change or quality reduction. Equal-rate output copies at most two contiguous
ring spans, retaining the original five-frame history/backpressure contract.
Inline annotations keep the copy branch from impairing FIR inlining.
No extra buffer, allocation or working-state field is introduced.
The measured production source SHA256 is
3d9f32ce7996b99b6f1aa81937c6d1b352697d9aee2287b59b1fdedbb9e964f5.

## Preserved reference and fidelity

baseline.rs is an immutable copy of the pre-optimization GamePKG0.1.17 module.
Its only adaptations are a coefficient-module path and provenance comment.
The unadapted source SHA256 was
a4047e9c5d6af2434bf0084363ad4268cb455aad4f6258c06cc1293e3948e800.
The adapted baseline and unchanged SDL2.30 coefficient table are locked in
source-lock.sha256. The full SDL notice is retained in the reference and the
original coefficient source. This is a historical implementation comparison;
the separate actual-SDL gates supply the independent behavioral oracle.

The differential compares every output bit for2592 streams/5963184 stereo
frames, across nine source rates, both output rates, four consumer sizes, nine
source lengths, irregular producer puts and two input profiles. Profiles include
normal PCM plus finite extreme/subnormal/negative-zero values. All output bits,
lengths and state sizes match; combined FNV64 is fc918c939505a13d on Linux64,
actual ELF32, actual PE32 under Wine and WebAssembly under Node/V8.

The current scalar state is16424bytes (mixer16512) on Linux64/PE32 and16420
(mixer16496) on ELF32, unchanged from each platform's baseline. The standalone
Wasm comparison has no host imports, a16MiB maximum and peaks at2031616bytes.
This comparison module is not shipped as a player companion or guest API.

The root audio_resampler test target also passes all nine tests against actual
SDL2.30.0, explicitly including normally ignored whole-stream and mono/stereo
staging/flush oracles. A new equal-rate FIFO test checks ring wrapping, signed
zero/subnormal/extreme bits, exact queue capacity, untouched output tails and
drained behavior. Rebuilt production decoder/converter Wasm retain their6MiB/
256KiB maxima. Native/browser consumers include this same source module.
These checks establish conversion behavior, not speakers, device scheduling
or whole-game audio/performance qualification.
The complete release/all-feature suite passes765 tests with160 explicit
ignores; the independent nine-test SDL target is also run with its ignored
oracles enabled. Strict all-target/all-feature release lint and formatting pass.
CLI/GUI release binaries build, and the GUI's noninteractive bundled-decoder
check confirms the rebuilt6MiB companion is embedded.

## Measured local results

The checked-in results contain31 alternating A/B rounds after warming both
implementations. Input creation is outside the timed section; each run converts
one second of synthetic input into a fixed800-frame scratch buffer. Output
buffer sizes are1,64 and800. Black-box consumption keeps the work observable.
No timing threshold is used as a correctness or CI assertion.

Host: AMD Ryzen5 9600X, Linux x86_64, Rust1.89.0/LLVM20.1.7, process pinned to
logical CPU2. Frequency and other system activity are not controlled. Each file
reports minimum/median/maximum times, sample counts and output frame counts:

| Compiler profile | Actual rate conversion,18 cases | Equal-rate playback,6 cases |
| --- | ---: | ---: |
| O3/native CPU/thin LTO/one codegen unit | 1.122–1.282x | 1.232–4.536x |
| O3/generic x86_64/16 codegen units | 1.147–1.210x | 1.264–6.303x |

Raw files: [native](results-native-x86_64-linux.jsonl),
[generic](results-generic-x86_64-linux.jsonl),
[unchanged-code control](control-x86_64-linux.jsonl).

These are median baseline-time/candidate-time ratios on this host, not promised
gains on other CPUs, in Wine, in the browser or in complete ports. Absolute
cost is small: typical44.1-to48kHz global conversion uses about0.6ms per second
of audio instead of about0.76ms at bulk consumer sizes. The unchanged-code
control also compares baseline/candidate before the edit and shows the ordinary
measurement variation. Use the raw rows, not a selected best sample.

## Reproduce

From the repository root:

    sha256sum -c tools/audio-resample-bench/source-lock.sha256
    rustc --edition=2021 -Dwarnings -C opt-level=3 -C target-cpu=native \
      -C lto=thin -C codegen-units=1 tools/audio-resample-bench/main.rs \
      -o /tmp/gamepkg-audio-resample-bench
    taskset -c 2 /tmp/gamepkg-audio-resample-bench --rounds 31

Select an available CPU or omit taskset on a different machine. Use --verify-only
for exact-output checks without timings. The generic profile replaces the
native/LTO arguments with -C codegen-units=16. For actual32bit processes:

    rustc --edition=2021 -Dwarnings --target i686-unknown-linux-gnu \
      -C linker=clang-18 -C link-arg=-m32 -C opt-level=3 \
      tools/audio-resample-bench/main.rs -o /tmp/gamepkg-audio-resample-elf32
    /tmp/gamepkg-audio-resample-elf32 --verify-only
    rustc --edition=2021 -Dwarnings --target i686-pc-windows-gnu -C opt-level=3 \
      tools/audio-resample-bench/main.rs -o /tmp/gamepkg-audio-resample-win32.exe
    WINEPREFIX=/tmp/gamepkg-steam-shortcuts-wine WINEDEBUG=-all \
      wine /tmp/gamepkg-audio-resample-win32.exe --verify-only

The restricted maintainer environment blocks a32bit Linux system call; the
same ELF32 comparison succeeds with authorized unrestricted execution. Wine
uses an isolated temporary prefix, never player application data.

    rustc --edition=2021 -Dwarnings --target wasm32-unknown-unknown \
      --crate-type cdylib -C opt-level=3 -C panic=abort \
      -C link-arg=-zstack-size=1048576 -C link-arg=--max-memory=16777216 \
      tools/audio-resample-bench/wasm.rs -o /tmp/gamepkg-audio-resample-compare.wasm
    node tools/audio-resample-bench/check-wasm.cjs /tmp/gamepkg-audio-resample-compare.wasm

CI contains the non-timing native/Wasm differential and source locks. Independent
SDL comparison remains in its existing gate. Results are not obtained by
changing tolerances, bypassing limits or changing the original recordings.
