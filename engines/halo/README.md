# Halo: Combat Evolved (OpenCE)

Linux host profile `opence-ce95f7de`. The selector is `ports/halo`. It is not listed in `ports.gamepkglist`.

## What works

The package is the same shape as Cube 2: a dependency-free launcher, a game-data folder, and no bundled commercial data. `gamepkg build` clones OpenCE `ce95f7deed5bbf726e50a9d5fe5227437194cf3b` into the library (or reuses `GAMEPKG_OPENCE_SOURCE` when that checkout is the pinned commit), checks the AGPL license digest, and runs upstream `configure.py --release --portable --pgo=off --lto=off` and `ninja linux` when `build/linux/halo` is not already a 32-bit ELF. The cache stores `halo`, `libSDL3.so.0`, and their sha256 receipts.

`gamepkg run` and `gamepkg-gui --play` start that binary. A folder with `maps/ui.map`, or exactly one `.iso`/`.xiso`, is required. Maps are symlinked from a private runtime `content/` directory so the player's folder stays put and OpenCE's log is written beside the link. A disc image is symlinked next to the executable, with `HALO_DATA_ROOT` unset, so the first launch extracts `maps/` into that runtime directory. `HALO_UPDATE_AUTO=false`. Saves use `HALO_SAVE_ROOT` there. When bubblewrap is installed, the process is started with `--unshare-net` because the manifest sets `network = false`, and `/` is read-only except for the runtime directory. Without 32-bit ALSA, SDL uses the dummy audio driver. `HALO_HIDDEN_WINDOW`, `HALO_NULL_RENDERER`, and `HALO_EXIT_AFTER` are forwarded when set.

## Decisions

- OpenCE is fetched and built, not committed. The decompilation and all Halo maps stay out of git.
- The engine is a separate 32-bit process with its own SDL window. A Cube 2-style pixel bridge is not part of this profile.
- The portable build needs network, so it is not the no-network bubblewrap compiler used for ordinary Rust packages. Playback can still drop the network through bubblewrap.
- Guest `network` stays false. OpenCE's own multiplayer sockets are not wired through the GamePKG broker.
- The hard memory cap is 3 GiB because the engine is 32-bit. The library memory ceiling must be at least 2048 MiB.
- There is no catalog entry. Publishing one needs a real release asset URL, which this repository does not have.

## Limits

Campaign, co-op, and multiplayer were not played here: this machine has no Halo disc image. The Linux binary does build, and a launch with a stand-in `ui.map` exits with OpenCE's own rejection of non-Xbox maps. Windows, Android, and the web builds of OpenCE are not GamePKG targets. Sound needs a 32-bit `libasound.so.2`, which Ubuntu's amd64 archive does not install unless the i386 architecture is enabled.

Upstream license: AGPL-3.0-or-later. It does not license Halo, its maps, or Microsoft's trademarks. The selector text in `ports/halo` is Apache-2.0.
