These maintainer tests compile the actual patched native reader and filesystem helpers. They generate synthetic disc images; no game data is needed or extracted. Python is used only by this test runner.

After applying the reviewed engine patches to the pinned source:

```sh
python3 engines/halo/tests/check_disc_view.py --source /path/to/engine
python3 engines/halo/tests/check_disc_view.py --source /path/to/engine --i686-sysroot /path/to/engine/build/linux/third_party/sysroot
```

The 64-bit run enables AddressSanitizer and UndefinedBehaviorSanitizer. The 32-bit run exercises the original native game ABI. Cases cover traversal, invalid extents, cyclic links, duplicate names, missing required data, damaged descriptors, truncation, sparse reads beyond 2 GiB, EOF, read-only descriptors, stale directory handles, and concurrent readers. Failures stop the run.

The HUD fixture recovers the pinned original scalar kernel and the adapted kernel from the reviewed patch. It needs no checkout, ROM, graphics device or game artwork:

```sh
python3 engines/halo/tests/check_hud_upscale.py --sanitize
python3 engines/halo/tests/check_hud_upscale.py --scalar
python3 engines/halo/tests/check_hud_upscale.py --cc clang --cflag=--target=i686-linux-gnu --cflag=-m32 --cflag=-msse2 --cflag=-mfpmath=sse --cflag=--sysroot=/path/to/engine/build/linux/third_party/sysroot
```

It compares every output byte and floating-point exception flag in 263,256 image cases across all four rounding modes, including exhaustive binary tap patterns, one-pixel edges, maximum dimensions and unaligned inputs. The default Linux Rust scaffold test runs both SIMD and scalar cases. `--source /path/to/engine` additionally verifies that the actual patched checkout contains the exact tested kernel. `--quick` and `--runner` support smaller cross-architecture fixture runs. Reported kernel timings are diagnostic; correctness does not depend on a machine-specific speed threshold.

`python3 engines/halo/tests/check_video_failure.py` checks normal, explicit headless, and standalone window-failure policies without game media.

The `Native Halo source safeguards` CI job fetches the pinned public source and applies the reviewed patches, then runs these malformed-image, sparse-offset, concurrent-read, exact-HUD and window-policy fixtures on actual native 64-bit and 32-bit code. It uses no game media. Python is a maintainer/CI tool, not a player requirement.

Movie fixtures generate an original three-frame black/white Bink clip and silent RDFT/DCT audio. They check every output pixel and padding byte, stride/offset bounds, frame timing, fractional audio samples across loops, mute/gain, cleanup/reopen, all four floating-point rounding modes, x87 precision and SSE state, bounded paths, malformed headers and truncated packets. A deliberately wrong i686 structure alignment must fail compilation. The 64-bit harness instruments both FFmpeg and the bridge with AddressSanitizer and UndefinedBehaviorSanitizer; the real game remains native i686.

`check_movie_view.py --source /path/to/patched/engine` separately checks maps/movie namespace separation, read-only metadata, traversal/ADS/NUL names, duplicate and cyclic directories, invalid extents, end-of-file and huge offsets, plus 8,000 concurrent reads. Its generated-disc cases run on native 32-bit and 64-bit sanitizer builds.

The following CI setup downloads only pinned open-source dependencies and builds dummy-audio SDL libraries for buffer tests. It opens no game window and needs no ROM:

```sh
python3 tools/check_opence_movie_codecs.py --source /path/to/patched/engine --work /tmp/new-movie32 --profile native32
python3 tools/check_opence_movie_codecs.py --source /path/to/patched/engine --work /tmp/new-movie64 --profile sanitized64
```

An existing reviewed player build can also supply its actual SDL library and prepared FFmpeg archives to `check_movie_playback.py`; see `--help`. These are decoder/buffer checks. GPU rendering, controller behavior and multiplayer require separate runtime checks.

The scripted-suspension fixture compiles the actual snapshot writer/reader, view application, transition handler, and original engine setter from the patched source. It checks 7,680 flag/shape/ownership/view round-trips and 291,816 truncated allocations per profile, including suspended units with camouflage, riding, damage and powerup fields. Thirteen application cases cover remote angular correction, active owner isolation, scripted owner cameras, angular velocities, non-cooperative isolation and rejection of invalid directions before partial writes. Parent/vehicle conversion is covered separately by the original-engine basis fixture. Repeated snapshots do not repeatedly clear velocity or airborne state, and unrelated flags survive. Three negative controls must fail the specific expected assertion: removing owner filtering, removing the non-cooperative guard, and disabling remote view application.

```sh
python3 engines/halo/tests/check_scripted_suspension.py --source /path/to/patched/engine --output /tmp/suspension32 --profile native32
python3 engines/halo/tests/check_scripted_suspension.py --source /path/to/patched/engine --output /tmp/suspension64 --profile sanitized64
```

The 64-bit fixture uses AddressSanitizer and UndefinedBehaviorSanitizer; it does not establish 64-bit gameplay support. Both fixtures use generated protocol values without game media. Network version 34 distinguishes the extended scripted-view snapshots from older peers.


The co-op runtime fixture extracts the patched prediction loop, simulation clock,
and vehicle-facing conversion. Each profile runs 26 clock cases (including the
17 earlier regressions), 14 scripted handoff cases, and 11,900 aiming-basis
round-trips using the original engine conversion and matrix functions. Six
negative controls per profile must fail their assertions: removing catch-up,
its rate guard, the ahead budget, fractional-frame preservation, scripted
position authority, or using the wrong parent-node camera basis.

```sh
python3 engines/halo/tests/check_coop_runtime.py --source /path/to/patched/engine --output /tmp/coop32 --profile native32
python3 engines/halo/tests/check_coop_runtime.py --source /path/to/patched/engine --output /tmp/coop64 --profile sanitized64
```

World, tag and device access are mocked in these asset-free fixtures. They prove
specific function behavior, including stale clock/invalid RTT handling, time
wrap, new game epochs, zero-tick render frames and original seat-basis rules.
Actual GPU playback, controller delivery, campaign progress, stalls, network
latency and frame times remain separate runtime checks. Python runs only in
maintainer tests; the player build and runtime do not use these scripts.
