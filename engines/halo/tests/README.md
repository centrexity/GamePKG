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
