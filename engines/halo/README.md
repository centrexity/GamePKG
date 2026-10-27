# Halo: Combat Evolved (OpenCE)

Profile `opence-ce95f7de` in the external-engine catalog. The selector is `ports/halo`, published as `com.gamepkg.halo` in `ports/ports.gamepkglist`. The shipped target is `linux-i686`. `windows-i686` is a cross-compile. The package contains no Halo assets and no OpenCE source.

The host does not special-case this game. The record is `engines/halo/engine.toml`. A package names the profile and repeats the reviewed data layouts. Commands, image formats, license checks, the desktop sandbox, and game-data placement come from `src/external_engine`, which loads every `engines/*/engine.toml` the same way. Another port can add a catalog file without a new host module. Host Rust does not name this engine. The platform gap list is [docs/external-engine-platform-gaps.md](../../docs/external-engine-platform-gaps.md).

## What the generic path does

- `gamepkg build --engine-target <id>` selects a catalog target. On x86_64 Linux the default is `linux-i686`, the ready target for this OS, because `linux-x86_64` is blocked.
- A blocked target returns the upstream reason and does not fetch or compile.
- The source is `GAMEPKG_ENGINE_SOURCE` or `engine-sources/<profile>`. The checkout must be the pinned commit, the license digest must match, and game-data filenames are refused.
- Configure and build arguments live in the catalog. `{vctools}` and `{winsdk}` expand from `GAMEPKG_MSVC_TOOLS` and `GAMEPKG_WINDOWS_SDK`.
- The cache is `builds/<digest>/<profile>/<target>/` plus a `current` file. The executable must match the catalog image (ELF class and machine, or PE machine and magic). Companions get their own sha256 receipts.
- Play links a `link` layout's directories into a private content directory and sets the catalog data-root variable. A `beside` layout symlinks one archive next to the executable and leaves that variable unset. Changing the selection clears the catalog's cleanup directories.
- With `network = false` on Linux, bubblewrap starts the process with `--unshare-net`, a read-only `/`, and a writable runtime directory. Without bubblewrap it warns and starts directly. Windows is a direct spawn. A cached target for another OS is not launched.

## OpenCE entry

`linux-i686` runs upstream `configure.py --release --portable --pgo=off --lto=off` and `ninja linux`. The cache stores `halo` and `libSDL3.so.0`.

`windows-i686` runs the same configure with `--portable`, `--windows-cross`, and the two toolchain directories, then `ninja windows`. After checkout the host applies `engines/halo/patches/0003-x86-arch-and-portable-march.patch`, which makes a portable i686 Windows build pass `-msse2 -maes -mpclmul`. Clang 18's `i686-pc-windows-msvc` target does not define `__SSE2__` for `-march=x86-64`. `-march=native` enables SSE2 and is not portable. Linux i686 portable stays on `-march=x86-64`. The cache stores `halo.exe` and `SDL3.dll`. The PE already cached from the earlier `-march=native` build still matches the image check, so the host does not reconfigure it.

`linux-x86_64` and `windows-x64` are catalog entries whose blocker is the upstream prefix. The same patch adds `configure.py --arch i686|x86_64` (default `i686`) and selects `x86_64-linux-gnu` or `x86_64-pc-windows-msvc`, `lib/x64/SDL3.lib`, and `-march=x86-64` for a portable 64-bit build. Those targets stay blocked until the prefix guards can go:

- `port/linux/include/halo_linux_prefix.h` errors unless the ABI is 32-bit x86, `HALO_ARM64_GUEST`, or wasm32. `tools/linux_build.py` hardcodes `--target=i686-linux-gnu -m32`. There is no x86_64 or x32 target. The Android guest is `arm64_32`, not 64-bit x86.
- `port/windows/include/halo_windows_prefix.h` errors unless `_M_IX86` is set. Clang `--target=x86_64-pc-windows-msvc`, `x86_64-w64-mingw32`, and `i686-w64-mingw32` all hit that error. Win64 is LLP64, so `long` is already 4 bytes and the remaining mismatch is pointer width. The upstream Windows target is `i686-pc-windows-msvc` only.

Patch `0001-pointer-independent-refs.patch` adds `halo_addr` (a 4-byte guest address) and static asserts for the Xbox on-disk sizes. Disc bytes stay 32-bit. Patch `0004-lp64-long-width.patch` stores Xbox `long` fields as `halo_s32`, which is 4 bytes on LP64 and is still `long` on i686 and Win64. Patch `0005-halo-addr-pointer-fields.patch` stores pointer fields in those size-asserted structs as `halo_addr` and loads them with `halo_ref`. The `#error` lines stay until a 64-bit binary passes the stand-in startup check. Patch `0002-gamepkg-host-log.patch` sends `platform_log` through `gamepkg_host_log` in `include/gamepkg_host.h`. A weak definition still writes stderr when the process is not linked to the host. There is no `centrexity/OpenCE` fork. The patches are applied by the catalog after the pinned checkout. They are not pushed upstream.

Verified here: the portable Linux i686 ELF builds, caches, and a launch with a stand-in `ui.map` exits with OpenCE's own rejection. A cross build with Clang 18, `llvm-rc`, `lld`, and an `xwin splat --arch x86 --use-winsysroot-style` SDK produced `halo.exe` (PE32, machine `0x14c`, magic `0x10b`, about 20 MB) and `SDL3.dll`. GamePKG cached that image and refused to launch it on Linux. It was not executed under Wine. Campaign, co-op, and multiplayer were not played: this machine has no Halo disc.

## Decisions

- OpenCE is fetched and built, not committed. The decompilation and all Halo maps stay out of git.
- The engine is a separate process with its own window. A Cube 2-style pixel bridge is not part of this profile.
- The portable Linux build needs network, so it is not the no-network bubblewrap compiler used for ordinary Rust packages. Playback can still drop the network through bubblewrap.
- Guest `network` stays false. Upstream multiplayer sockets are not wired through the GamePKG broker.
- The hard memory cap is 3 GiB because the engine is 32-bit. The library memory ceiling must be at least 2048 MiB.
- There is no catalog entry. Publishing one needs a real release asset URL, which this repository does not have.

Upstream license: AGPL-3.0-or-later. It does not license Halo, its maps, or Microsoft's trademarks. The selector text in `ports/halo` is Apache-2.0.
