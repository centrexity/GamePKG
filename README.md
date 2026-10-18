# GamePKG — Play games safely

[Website and player guide](https://centrexity.github.io/GamePKG/)

**Know what you run. Control what it can access.** GamePKG is building a safer way to share and play games: inspectable source, creator-signed packages, isolated compilation, and a host that is designed to grant only the capabilities a game needs.

**Current status:** this is a development preview. Package verification, catalog import, installation, update checks, and isolated Cargo Wasm and Slang shader builds on Linux and the configured Windows WSL 2 connection work today. The default Linux build uses a direct CPU executable in an isolated native worker. Optional Wasm profiles play in the GUI and terminal under Wasmtime with scoped asset reads, memory limits, and epoch deadlines. Vulkan is the default renderer; bounded Slang fragment passes use WGSL or SPIR-V, with OpenGL available explicitly; versioned WIT components are supported alongside the legacy core profile; broader rendering remains pending; this is not a guarantee that a package is harmless.

Download the [0.1.17 Linux and Windows CLI/desktop apps](https://github.com/centrexity/GamePKG/releases/tag/v0.1.17), including the checksum for your platform. Windows source builds use the matching Linux compiler inside WSL 2; playback runs in the native Windows app. See [Windows setup](docs/INSTALL_WINDOWS.md). Host distributions can also be prepared with `python3 tools/package_release.py /tmp/gamepkg-distribution` after building both release entry points. The archive includes CLI/GUI executables, setup tools, examples, original resolved dependency sources and notices, and integrity hashes. See [installation instructions](docs/INSTALL.md). See the [implementation and verification audit](docs/IMPLEMENTATION_STATUS.md) for requirement coverage and remaining limits. The public repository's **Package Linux preview** workflow produces the same artifact on Ubuntu 24.04.

Try the [signed demo catalog](https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demos.gamepkglist) by choosing **Add** in the desktop library, then **Import .gamepkglist**. It adds Collect Stars, Shape Playground, PNG Sprite Playground and Zelda3 as **Not downloaded**. Choose a game to download, approve its creator on first installation, then **Build** and **Play**. The three original demos need no ROM; Zelda3 requires your supported USA ROM. Players need no signing key. The [portable demo store](https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demos.gamepkgstore) includes all four signed games for offline import. Compare downloads against [demo checksums](https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demo-packages.sha256).


## Why distribute GamePKG instead of an opaque executable?

A conventional game executable can be difficult to inspect, and launching it without a sandbox may give it the permissions of your user account. That can include reading personal files, making network connections, or starting other programs. Signed binaries and separately sandboxed applications can also provide protections; the benefit of GamePKG is making source visibility, package verification, restricted builds, and declared capabilities part of one transparent workflow.

A `.gamepkg` contains Rust source, Slang shader source, assets, and a signed manifest. You can inspect the code and declared permissions before choosing to install it. The host checks the package, compiles its supported source profile into Wasm, and keeps game assets in the archive. The runtime exposes scoped game operations through the host rather than the operating system’s full set of interfaces.

| Protection | What it changes | Available today? |
| --- | --- | --- |
| Inspectable source | Games ship source rather than a bundled native executable; review is possible before building. Source visibility alone does not prove safety. | Yes |
| Signed identity and updates | The manifest and package contents are signed. Updates must keep the installed creator key and package ID, with a higher version. | Yes; first creator approval is still required |
| Restricted package contents | Rejects traversal paths, duplicate entries, symlinks, oversized archive contents, unsupported shaders, and source outside the initial policy. | Yes |
| Isolated compilation | Supported Linux builds run inside bubblewrap with a private work area, no network, and no access to your home directory. | Yes; checked source modules with no Cargo dependencies |
| Limited game access | Games receive host-approved asset, input, ROM, drawing, audio, save and endpoint-scoped network operations. | Available in the native worker and optional core/WIT Wasm profiles |
| Runtime resource limits | Memory caps and deadlines constrain running games. | Enforced for native workers and optional Wasmtime profiles; GPU work is outside CPU time limits |

## Build locally, improve over time

Building on demand on your own machine creates opportunities to optimize games for the supported platform and runtime instead of relying on a single prebuilt executable. Shipping source also lets the host rebuild games as compiler tooling and optimization settings improve. Creators can release signed source updates, and the host can check periodically for newer versions while preserving creator-key continuity.

These are opportunities, not a promise that every local build will be faster. GamePKG supports isolated native and optional Wasm builds, manual rebuilds, user-requested update checks, and configurable periodic checks while the desktop library is open and idle. Broader platform-specific performance tuning remains planned. Wasm builds must stay within their supported target and sandbox; local compilation does not grant games extra access to your machine.

## What can a GamePKG do—and what can it access?

Today, importing a `.gamepkglist` adds catalog entries without fetching games or trusting creators. Downloading fetches the package you select. Installation verifies and stores it; it runs no game code or package-provided install scripts. Update checks stage a signed candidate for review. Supported builds compile the verified Rust source to Wasm inside an isolated Linux process.

The package policy supports checked Rust source modules with no Cargo dependencies. Scoped functions are injected by the host. It rejects build scripts, proc-macro configuration, compile-time file includes, unsafe blocks, foreign declarations, and filesystem, network, process, or environment paths detected by the source scanner. Shader files must be Slang source. The scanner is a policy filter, not a substitute for compiler isolation or a runtime sandbox.

The runtime provides **no ambient filesystem or network access**. Games use specific capabilities such as input, audio, or saving through the host. Asset access uses virtual paths inside the game’s archive; save access is scoped by creator, game ID, and save schema. A game requiring a ROM declares that requirement, and the player supplies it. ROM bytes are shared read-only only for the selected play session. Optional signed file permissions allow import/export through an explicit player file choice; games receive bytes and a basename, never a host path. See [player-selected files](docs/DEVELOPING_GAMES.md#player-selected-import-and-export).

Game networking is off by default. A signed `network = true` enables supported HTTPS GET and TCP/UDP operations, including listeners, for internet and local-network addresses without endpoint allowlists; `false` disables them. See [networking permissions and API](docs/NETWORKING.md). User-requested host downloads over HTTPS or Tor are separate from game permissions. A signed update URL does not give the game permission to browse the internet. Signatures establish integrity and continuity with a creator key, not a creator's real-world identity or a promise that their code is safe.

Port recipes for DOOM, Zelda3, Arkanoo, and 2048 live in [GamePKG_Ports](https://github.com/centrexity/GamePKG_Ports). This repository pins them under `ports` for integration tests and release bundles. Existing checkouts should run `git submodule update --init --recursive` before building or testing. Trusted engine integration remains part of the host.

To import the published ports, choose **Add** in the library, then **Import official ports library**. The list URL is remembered for future refreshes. Or download [ports.gamepkglist](https://github.com/centrexity/GamePKG_Ports/releases/latest/download/ports.gamepkglist) and drag it into the app. Entries remain **Not downloaded** until you choose a game; packages still require creator review. The current DOOM port requires the updated main host.


## Try it

The desktop **Support** tab and [website support page](https://centrexity.github.io/GamePKG/support/) offer PayPal, Bitcoin, Monero, Dogecoin, and a link to the GamePKG mining group. Payment details are currently blank. Maintainers can add them in `html/support/details.json`; the site reads that file and the app embeds it when rebuilt. Click a configured crypto address to copy it. `gamepkg-gui --support` opens the page directly.

Install Rust, clone with the pinned ports, then run:

```sh
git clone --recurse-submodules https://github.com/centrexity/GamePKG.git
cd GamePKG
cargo test --locked
cargo run -- --help
```

A complete package input is in `examples/minimal`. Generate a creator signing key with `keygen`, keeping it outside the package directory. The command uses operating-system randomness, refuses to replace an existing file, and prints only the public creator key. On Unix, the private file is readable and writable only by its owner. Keep a private backup: signed updates must use the same key. Players do not need this private key.

```sh
cargo run -- keygen -o /path/to/signing.key
cargo run -- pack examples/minimal -o minimal.gamepkg --key /path/to/signing.key
cargo run -- verify minimal.gamepkg
cargo run -- asset minimal.gamepkg hello.txt -o hello.txt
```

Packing validates before publishing and refuses to overwrite files. The output must be outside the input directory. Signing keys are never stored in packages. The public key is inserted into the signed manifest during packing. Verification checks the embedded signature automatically; first installation requires a creator trust decision. An embedded key establishes signature integrity, not independent real-world identity.

## Initial policy

Game source also follows an enforced [portable data policy](docs/PORTABILITY.md): fixed-width byte conversions, explicit byte order, a fixed wasm32 ABI, and no native layout or pointer-dependent serialization. Dedicated 32-bit Linux, Windows, and Android checks exercise the same core and component runtime boundaries.

- Format version 1; networking is denied unless explicitly enabled, with bounded host operations; positive memory limits (up to 512 MiB for ordinary games, with larger bounded limits for reviewed native-engine profiles).
- Required manifest, PNG-signature thumbnail, license, credits, Cargo manifest/lockfile, and `source/src/lib.rs`.
- At most 10,000 entries, 64 MiB per file, 256 MiB uncompressed total; stored and zstd entries only.
- No absolute paths, traversal, backslashes, duplicate paths, directory entries, or symlinks.
- Only UTF-8 `.slang` files under `shaders/`. Inline SPIR-V, target-code injection and macro token pasting are rejected in every shader file, including unused sources. The conservative check also inspects comments and strings; it supplements the isolated compiler and does not prove GPU code is harmless.
- Rust source files under `source/src/`, no Cargo dependencies, `package.build = false`, and restricted Cargo fields. Includes, attributes, unsafe blocks, foreign declarations, and filesystem/network/process/environment paths are rejected.
- Optional AV1/WebM and Theora/Ogg previews undergo isolated codec, size, duration, frame-count, and initial-keyframe validation. PNG package validation checks magic bytes; GUI image decoding is bounded.

Filesystem access from games is virtual and scoped: assets come from the signed
package, ROM reads use only files explicitly selected by the player, and saves
use the game's own bounded save data. Games receive no operating-system paths,
file handles or a general filesystem API. Networking uses the endpoint-scoped host API. Package downloading and
update checks run in the host separately from game execution. Core Wasm imports
must be approved GamePKG functions; component imports must be functions from the
versioned GamePKG host interface. Filesystem, socket, and unknown imports are
rejected before guest initialization. This boundary denies ambient access rather
than replacing Rust's standard library with a permissive compatibility layer.

The source scanner is an early policy filter, **not a security boundary**. Rust macros and compiler behavior require an OS-isolated build process, followed by a Wasm capability sandbox. Linux builds now invoke the verified Cargo compiler inside bubblewrap; game code executes only through the Wasmtime runtime when Play/run is requested. Do not relax the source profile before those boundaries exist.

## Implemented workflow and remaining work

Archive verification, signed creator continuity, source checking, Linux isolated builds, and all four requested Slang output formats are implemented. CLI and desktop workflows support installation, catalogs, updates, build/rebuild, and scoped gameplay. The desktop includes previews, Settings, and portable store imports with shared assets. Core and versioned WIT component runtimes enforce memory and tick deadlines.

Local tests cover accepted and rejected packages, every good source example, compiler isolation, scoped runtime operations, store transfers, and desktop behavior. Hosted verification runs the same fixture integrations. The [quickstart](html/guide/index.html) explains installation and play; the [library policy](html/guide/allowlist.html) describes the no-dependency source policy.

Remaining development includes broader GPU resources and vertex/compute pipelines, native Metal/DXIL desktop rendering, and isolated source builds on other operating systems. Key rotation, automatic save migration, and shared-pool garbage collection are also unfinished. The current Linux preview supports the documented gameplay profiles; these limits remain visible to creators and players.

See [the format notes](html/spec/index.md) and [security design](html/guide/security.md). The original design brief is preserved in `DESIGN.md`. No site has been deployed.

## Creator signing and updates

Creators sign their `.gamepkg` files with their own Ed25519 private keys. Verification proves the package was signed by the holder of a trusted key and that its signed contents have not changed. A signature alone does not prove a person's real-world identity: the public key must first be associated with the creator through a trusted channel.

The installer pins the creator key to the installed package ID. Updates must keep that ID, use the same key, and have a strictly higher semantic version. Key changes are rejected even with `--trust-creator`. Rotation and lost-key recovery remain pending. `verify` checks the embedded key by default; `--trusted-key` remains available for advanced callers who require a separately known signer.


## License and attribution

GamePKG and its minimal example are licensed under [Apache 2.0](LICENSE). [NOTICE](NOTICE) identifies the project and links to https://gamepkg.com. When redistributing covered code or derivative works, preserve applicable copyright and attribution notices, include the license, and mark modified files as required by its terms. NOTICE attribution may appear in distributed notices, source/documentation, or the appropriate notice display; the license does not mandate a clickable or prominently displayed backlink on every website.

Creators choose their own licenses for their games and assets. Using the GamePKG format or host does not automatically license a game under Apache 2.0. The package verifier accepts an optional top-level `NOTICE`, includes it in the signed tree, and does not require it for independently authored games.

## CLI and desktop GUI

Build both entry points locally:

```sh
cargo build --locked --release --features gui --bins
cargo test --locked --all-features
```

The executables are `target/release/gamepkg` and `target/release/gamepkg-gui` (with `.exe` on Windows). Launch the GUI with `cargo run --locked --features gui --bin gamepkg-gui`. It lists installed packages and accepts a local package path or HTTPS URL. Inspection runs in a background worker and displays the verified creator fingerprint and permissions before installation. New creators require approval; updates use the installed creator key automatically. It compiles the supported source profile on Linux and plays core and versioned WIT component games with scoped assets, ROMs, saves, drawing, and audio. Resource-free fragment passes render through OpenGL; broader graphics remain unfinished. A desktop display is required to launch it.

Game windows remember their normal size, fullscreen and maximized state separately
for each game and creator in the selected library. F11 toggles the actual window
mode where supported by the game. Fullscreen/maximized and minimized bounds do
not replace the remembered normal size. The library window keeps its own defaults.
Invalid window records fall back to defaults and remain preserved; window settings
are host-owned metadata, never a guest filesystem permission or packaged game data.

The CLI remains available without desktop dependencies: `cargo build --locked --release --bin gamepkg`. See [examples](examples/README.md) for valid and deliberately rejected packages. Local integration tests exercise the actual CLI binary, every example, and signed archive tampering.

## Install, download, and update

```sh
gamepkg verify game.gamepkg
gamepkg install game.gamepkg --trust-creator
gamepkg install https://creator.example/game.gamepkg --trust-creator
gamepkg list
gamepkg update com.creator.game
```

Without `--trust-creator`, a first CLI install reports the creator fingerprint and waits for an explicit re-run accepting that creator. The GUI has a “Trust this creator and install” button. Existing packages need no new trust approval. Neither an embedded key nor a URL certifies a creator's name; obtain the first package from a source you trust.

Creators may set `update_url = "https://creator.example/latest.gamepkg"` at the top level of their input manifest. It points directly to the newest signed `.gamepkg`, not a JSON feed. The host downloads it only when the user requests an update; the downloaded ID, signer, and version must pass continuity checks. Packing inserts `creator_key` automatically; if supplied, it must match the signing key. The private signing key never goes inside the archive.

Downloads require the `curl` executable, enforce HTTPS for ordinary hosts, Tor-only routing for onion hosts, certificate verification for HTTPS, connection/transfer timeouts, and a 256 MiB transfer limit. No game code runs during installation. Host downloads are separate from guest network permissions, which are separately declared in the signed manifest.

Installed archives reside under `GAMEPKG_HOME` when set, otherwise `$XDG_DATA_HOME/gamepkg`, `%LOCALAPPDATA%/gamepkg`, or `$HOME/.local/share/gamepkg`. Installation snapshots and verifies the bytes before replacement, uses an install lock, and leaves the old version intact on validation failure. An interrupted process may leave `.install-lock`; remove it only after confirming no install process is active.

For a local HTTPS end-to-end test (requires Python 3, OpenSSL, and curl), run `python3 tests/https_download.py` after the release build. It creates temporary certificates and packages, verifies URL installation and updates, and checks signer and ID substitution rejection. The server and temporary keys are cleaned up afterward.

## Library table, build actions, and ROM requirements

The desktop library shows each installed package's thumbnail, ID, version, ROM requirement, build status, and actions. Build becomes Rebuild once the exact installed package has a cached Wasm output. Installing a new package version resets that status. Play is enabled for built packages; ROM games require a file path in the library’s ROM field. The package must implement the grid-v1 entry point; packages without it report a clear runtime error and need a playable source implementation. Thumbnails decode with bounded dimensions and allocation; invalid icons show a fallback.

Set `requires_rom = true` at the top level of the manifest for a game requiring a ROM supplied by the end user. It defaults to false for existing packages and is covered by the creator signature. The host displays the requirement during inspection and in the library. No ROM is downloaded, uploaded, or included automatically. Select a local ROM path when launching; the guest receives bounded read-only bytes for that session.

`gamepkg build <id>` and the GUI Build action now compile native machine code for the current Linux computer, with a separately isolated worker. They do not also build Wasm. See [native builds, isolation and Vulkan rendering](docs/NATIVE_RUNTIME.md). Use **Build Wasm** or `gamepkg build <id> --target wasm` for the optional Wasm profile. That profile compiles verified `source/src/lib.rs` with host-supplied bindings through offline, locked Cargo to `wasm32-unknown-unknown` under Linux bubblewrap, without Cargo build scripts. It requires `bwrap`, `timeout`, `prlimit`, Rust, `rustup target add wasm32-unknown-unknown`, and the pinned Slang installation described below. The compiler has a private working directory, an isolated network namespace, access to system tools and its toolchain, and a 60-second deadline. It has no access to the user's home directory. Failed sandbox setup stops the build; there is no unsandboxed fallback. Compiler processes have a 2 GiB virtual address-space ceiling, core dumps disabled, output-file limits (32 MiB for Rust, 16 MiB for Slang), and bounded diagnostics (1 MiB per stream). These are per-process limits rather than a total process-tree memory budget.

`gamepkg check-updates <id>` checks the signed URL without changing the installed game. In the GUI, a newer update opens for review and explicit installation. Matching versions report up-to-date; older versions and changed IDs/keys are rejected. `gamepkg update <id>` remains the explicit download-and-install command.

After building the host, run `python3 tests/isolated_build.py` on a Linux machine with bubblewrap support to verify isolated compilation and rebuilding.

## Tor onion downloads and updates

`update_url` and the download field accept `http://<v3-address>.onion/latest.gamepkg` and HTTPS onion URLs. Ordinary hosts still require HTTPS. HTTP onion transport is permitted because onion-service connections are protected by Tor; package signatures and creator continuity checks still apply.

Run a local Tor SOCKS service on `127.0.0.1:9050`, or set `GAMEPKG_TOR_PROXY=127.0.0.1:9150` for a different local listener. This variable accepts a loopback IP and port, not a URL. The CLI and GUI both use it. GamePKG does not bundle or start Tor.

Onion downloads explicitly use `socks5h`, so the proxy resolves onion names. `NO_PROXY` cannot bypass this route; proxy failure stops the download with no direct fallback. Redirects are validated individually, and onion-to-ordinary-host redirects are rejected. Automatic curl configuration files are ignored. HTTPS onion URLs retain certificate verification.

Run `python3 tests/tor_download.py` after building to test remote DNS, Tor proxy routing despite `NO_PROXY=*`, signed installs/updates, and redirect rejection with a mock SOCKS5 server. This test does not contact or certify connectivity to the live Tor network.

## Bulk game lists and the not-downloaded state

A `.gamepkglist` is a UTF-8 TOML catalog, not an archive. Import adds game metadata to the library without downloading game packages, thumbnails, ROMs, or granting creator trust. Remote imports download only the list itself. See [starter.gamepkglist](examples/lists/starter.gamepkglist) for a complete example with placeholder URLs.

```toml
format_version = 1

[[games]]
id = "com.creator.game"
title = "Example Game"
download_url = "https://creator.example/game.gamepkg"
version = "1.0.0"     # optional catalog claim
requires_rom = false # optional catalog claim
```

```sh
gamepkg import-list starter.gamepkglist
gamepkg import-list https://creator.example/games.gamepkglist
gamepkg list
gamepkg download com.creator.game --trust-creator
```

Use the Library **Search** box to match game titles, IDs, and descriptions as you type. Searches ignore capitalization; multiple words must all match, and **Clear** restores the full list within the selected status filter. Search is kept while switching pages during the current session. Large libraries show 20 games per page by default, with **Previous page** and **Next page** controls above and below the games. Choose 10, 20, 50, or 100 games per page. Search and filters apply to the entire library and return to the first page when changed. Page changes scroll back to the top; refreshing a smaller library keeps the selected page within the available range. The library filters games by **All games**, **Ready to play**, **Needs build**, **Needs ROM**, or **Not downloaded**. Ready to play checks the native build and required local files; Needs ROM includes missing ROM selections and catalog entries marked as requiring a ROM. **Refresh library** reloads local packages and updates every saved list URL without downloading game packages. The **Add** page lists saved URLs, with **Update list** and **Remove list** actions. Removal asks for confirmation and removes only that source’s catalog entries; installed games and entries provided by other lists remain. Failed updates keep the last successful entries while other lists continue refreshing. Local file imports stay offline. Previously imported URLs must be imported once more to remember them, since older versions did not retain their addresses.

The **Add** button on the library opens a separate page with the path/URL field and **Import .gamepkglist** action. Imported games appear in the table as **Not downloaded**, with a placeholder icon and **Download / inspect** action. Version and ROM claims from the catalog are unverified; the signed manifest becomes authoritative after downloading. The package ID must match the requested catalog ID, and creator approval is still required before first installation. Existing installed packages appear only once and retain their verified metadata and signer continuity rules.

Lists support the same HTTPS and Tor onion URLs as packages, have a 4 MiB/10,000-game limit, reject unknown fields and duplicate IDs, and are imported atomically. Reimporting identical entries is harmless. Conflicting metadata for existing catalog IDs rejects the entire import rather than silently replacing download sources. Catalog metadata persists in `catalog.gamepkglist` under the library directory; it contains no game binaries or creator trust approvals. A crashed import may leave `.catalog-lock`; remove it only after confirming no import process is active.

## Play the first game

After building both release binaries, run `./play-demo.sh`. It signs the bundled example with a temporary local demo key, installs it in `.demo-library`, builds it in isolation, and opens the desktop player. The private demo key is deleted; this separate demo library does not change your normal installed games.

`examples/good/collect-stars` is a playable guest whose movement and scoring run inside Wasmtime. Pack and install it using your creator key, then build and play:

```sh
gamepkg pack examples/good/collect-stars -o collect-stars.gamepkg --key /path/to/creator.key
gamepkg install collect-stars.gamepkg --trust-creator
gamepkg build com.gamepkg.collect-stars
gamepkg run com.gamepkg.collect-stars
```

In the terminal, enter W/A/S/D to move and Q to quit. In the GUI, click Play and use arrows or WASD; collect yellow targets to increase your score. Restart resets the game; Back to game unloads it and returns to that game’s page. The desktop executable also accepts `--library <directory> --play com.gamepkg.collect-stars`.

The legacy grid-v1 ABI uses core Wasm. The versioned WIT component profile is described below. Guest source defines `pub fn game_tick(input: u32, state: u64) -> u64`; the host compiler appends the exported wrapper, so guest export attributes are unnecessary. State packs player x/y and target x/y into four bytes, followed by a 32-bit score. Input bits are left=1, right=2, up=4, down=8. Coordinates use a 16×16 grid. Guests receive zero input unless the signed manifest grants input.

Runtime modules can import only the scoped GamePKG asset, ROM and save APIs; WASI, filesystem and socket imports are rejected. Wasmtime enforces memory and epoch limits. A trap ends gameplay. Slang compilation is available; Resource-free GLSL fragment passes render in the desktop player.

For a game declaring `requires_rom = true`, use `gamepkg run <id> --rom /path/to/file` or open the game’s page and click each ROM file field to choose it with the system file picker. ROM files must be regular files of 1 byte to 64 MiB and are read into a session snapshot. Guests use `gamepkg::rom_size()` and `gamepkg::rom_read(offset, &mut buffer)`, never a host path. Reads are capped at 64 KiB. No ROM is downloaded, installed or bundled automatically. `examples/good/rom-required` is a playable data-driven example; `python3 tests/runtime_rom.py` tests the signed build and runtime path.

Run `python3 tests/playable.py` after the release build to test the complete signed-package, isolated-build, movement and scoring path. Runtime tests separately exercise forbidden imports, missing exports, input permission denial, excessive memory, infinite loops and epoch interruption.

## Recommended memory and hard memory limit

Creators set both values in the signed manifest:

```toml
[limits]
recommended_memory_bytes = 8388608 # 8 MiB, advisory
hard_memory_bytes = 16777216       # 16 MiB, enforced
```

Recommended memory describes the creator's suggested budget; it neither reserves memory nor stops a game from using more. Wasmtime enforces the hard limit on guest linear memory, including initial allocation and growth. It is not a cap on the entire host process, compiler, or JIT memory. The GUI displays both values during inspection and in the installed-state tooltip.

Recommended memory is optional and defaults to the hard limit. When supplied, it must be positive and no greater than the hard limit. The hard limit for ordinary games must be positive and at most 512 MiB. Reviewed native-engine profiles can define a larger bounded limit; this does not increase arbitrary guest Wasm memory. Existing `memory_bytes` remains accepted as an alias for `hard_memory_bytes`; supplying both is rejected, and newly packed manifests use the canonical hard-limit field.

On each installed game’s page, unmet launch requirements appear directly beside the Play controls: a missing or invalid native build, required memory ceiling, unavailable game-data folder, or missing named ROM files. Build completion refreshes these checks automatically. Saving settings or choosing files updates readiness; **Recheck readiness** reloads externally changed build or file selections. Memory ceilings are never raised automatically.

## Desktop settings

The **Limit game render resolution** option in Settings is off by default. Cube 2
renders at the game window's physical resolution unless you enable the option
and choose maximum width and height. An enabled limit preserves aspect ratio;
changes apply the next time the game launches.

Open **Settings** from the desktop library navigation. Save applies validated preferences; Cancel discards form changes, and Reset restores defaults in the form until you save.

**GamePKG updates** in Settings checks the latest stable release from the official Centrexity/GamePKG repository for your operating system and CPU. Choose **Download and install GamePKG update**, select a writable folder, then **Restart into updated GamePKG**. Downloads are checked against the release's SHA-256 checksum, and executable hashes and build identity are checked before installation. Archive links, unsafe paths and excessive expanded sizes are rejected. These checks provide download integrity through the official HTTPS release channel; they are not an independent publisher signature.

The updater installs a new portable version folder and keeps the current copy for rollback. Update your shortcuts to the new copy. Games, ROM selections, saves and settings remain in their existing locations. Close running games before restarting. No administrator access, Python or WSL is needed to update the app. A missing release for your platform, failed download or failed verification leaves the current installation intact. GamePKG checks for app updates in the background when the library opens and shows a notice when a newer stable release is available. To disable this, clear **Check for app updates when GamePKG opens** under **Settings → GamePKG updates** and save. Manual checks remain available. Game-only windows do not check for app updates. Failed startup checks do not interrupt library work; details appear in Settings. This preference is separate from periodic game update checks.

- Library folder: choose an absolute path or leave empty for the default. Switching libraries does not move existing packages.
- Tor proxy: local loopback IP and SOCKS port; Tor remains a separately running service.
- Game ceilings: memory (1–4096 MiB, default 512 MiB), tick deadline (10–1000 ms, default 100 ms). Memory uses the lower of the saved ceiling and the signed game limit. New sessions use the settings; current games are unaffected.
- Optimize builds: defaults to high native optimization for subsequent Build/Rebuild: Rust `opt-level=3`, thin LTO, one codegen unit, and `target-cpu=native`; native C/C++ engines use `-O3` and native CPU tuning. Disabling it uses level 0 for troubleshooting. Wasm export keeps its separate portable optimization profile. Rebuild existing games to apply the new settings.
- Appearance: dark or light theme, applied after saving.

Settings persist in `$XDG_CONFIG_HOME/gamepkg/settings.toml`, `%APPDATA%/gamepkg/settings.toml`, or `$HOME/.config/gamepkg/settings.toml`. Set `GAMEPKG_CONFIG` to override the config file. `GAMEPKG_HOME` (including GUI `--library`) and `GAMEPKG_TOR_PROXY` override the saved library and proxy respectively. The CLI shares library, proxy, build and runtime settings. Invalid configuration is reported rather than silently weakening restrictions.

## Guest asset reads

The isolated compiler supplies `gamepkg::asset_read(path, offset, &mut buffer)`.
Paths are relative to `assets/` in the signed archive. The function returns
`Some(bytes_read)`, including zero at EOF, or `None` for invalid paths, offsets,
or missing assets. Reads are limited to 64 KiB per call; archive work consumes
the host time and memory boundaries. Verified stored assets use direct archive offsets; compressed assets
stream through the prefix without allocating the entire file. The guest receives only the requested bytes and cannot open host
files. The same safe Rust wrappers are available in the versioned WIT component
profile. Rebuild existing games to use the new wrapper.

Run `python3 tests/runtime_assets.py` after building the release CLI to verify
asset reads and path rejection through signing, isolated compilation and play.

## Scoped saves

Games declaring `permissions.save = true` can use `gamepkg::save_size()`,
`gamepkg::save_read(offset, &mut buffer)` and `gamepkg::save_write(&bytes)`.
The host keeps one ordinary-game blob of at most 1 MiB under the library’s `saves` folder,
scoped by the signed game ID and creator key. Same-creator updates retain the
blob; another game or creator cannot select it. Reads are capped at 64 KiB.
Denied permissions return `None` or `false` without opening save storage.
Writes replace the blob atomically only after a successful
tick. A trapped tick discards its pending save. Concurrent sessions use the
last successfully committed write. The version-one blob format belongs to the
game; creators should handle migration when changing their save layout.

The reviewed DOOM engine has a separate, permission-checked 4 MiB bundle allowance
for six original save slots and two configuration files. Ordinary guest save APIs
retain their 1 MiB limit. Old raw slot-0 saves migrate without changing their
contents; the new bundle requires an updated engine build.

`examples/good/save-counter` keeps its tick count across sessions. After a
release build, `python3 tests/runtime_saves.py` verifies persistence, updates,
permission denial and trap rollback.

## Pinned shader builds

Build/Rebuild now compiles the signed Slang shader source in a network-isolated
Linux process before publishing the Wasm artifact. Install the supported
compiler into a new folder:

```sh
python3 tools/install_slang.py /absolute/path/gamepkg-slang
export GAMEPKG_SLANG_ROOT=/absolute/path/gamepkg-slang
```

The installer downloads the official Slang 2026.19 Linux x86-64 release and
checks a pinned SHA-256 before unpacking it. Alternatively set the installation
folder and backend in the desktop Settings page. New configurations default to
WGSL or SPIR-V for Vulkan shader playback. OpenGL also accepts GLSL. SPIR-V, WGSL, Metal,
and DXIL compilation outputs are also tested. DXIL requires the pinned
DXC 1.9.2609 Linux x86-64 distribution:

```sh
python3 tools/install_dxc.py /absolute/path/gamepkg-dxc
export GAMEPKG_DXC_ROOT=/absolute/path/gamepkg-dxc
```

The installer verifies the official release archive checksum. The host checks
both DXC library hashes before compilation, mounts them read-only inside the
build sandbox, and includes the pinned DXC identity in the shader cache key.
Missing or modified libraries fail the build. No backend fallback occurs. Select GLSL, SPIR-V, or WGSL and rebuild to use resource-free
fragment passes in the OpenGL desktop player. SPIR-V is parsed and validated by
pinned Naga 24.0.0, then translated to host-owned GLSL 330 before graphics setup.
Metal and DXIL rendering integration remains pending.

Legacy manifests default to `shaders/main.slang`, `fragmentMain`, fragment
stage. Declare multiple entry points explicitly using signed manifest entries:

```toml
[[shaders]]
source = "shaders/main.slang"
entry = "fragmentMain"
stage = "fragment"
```

Supported stages are vertex, fragment and compute, with up to 32 entries.
Compiler outputs and JSON reflection are cached outside the archive, keyed by
package bytes, compiler version/binaries and backend. A failed shader build
publishes no partial cache or new playable artifact. The compiler sees its
own tools, system libraries and staged shader sources; your home directory
and network are unavailable. Run `python3 tests/shaders.py` with
`GAMEPKG_SLANG_ROOT` set to verify actual backends, reflection and host include
rejection.

## Guest drawing

Guests can draw their own scene with `gamepkg::draw_clear(rgba)`,
`gamepkg::draw_rect(x, y, width, height, rgba)` and
`gamepkg::draw_circle(x, y, radius, rgba)`. These safe wrappers send bounded
commands to the desktop renderer; games never receive a graphics-device handle.
Coordinates use a 640 × 480 canvas and colors use `0xRRGGBBAA`.
Each frame keeps at most 1,024 commands. Non-finite coordinates and oversized
geometry are rejected, and trapped ticks discard the frame.
The desktop clips commands to the game viewport and scales the canvas to fit.

Input bits 0–3 report newly pressed left/right/up/down directions as before.
Bits 8–11 report held directions (`HELD_LEFT`, `HELD_RIGHT`, `HELD_UP`,
`HELD_DOWN`). Disabling input permission clears both sets of bits.

`examples/good/shape-playground` draws an animated scene and moves a square
with held arrows/WASD. Play it through the GUI or run it headlessly using
`--frames`. Interactive terminal playback remains for the grid profile.
`tests/drawing_game.py` checks the signed source/build/runtime path;
`tests/drawing_gui.py` uses a virtual display and actual screen pixels to
verify rendering and held-key movement. The latter requires Xvfb, xdotool,
ImageMagick and Pillow. Set `GAMEPKG_SLANG_ROOT` for both tests.

The host also exposes `gamepkg::draw_shader(index)`, which draws a full viewport
pass using the signed manifest's fragment entry at that index. Select GLSL, SPIR-V, or WGSL in
Settings and rebuild. The host checks reflected resources and active uniforms,
keeps graphics handles private, and bounds the per-tick command queue. Guest
Wasm deadlines do not interrupt GPU shader execution; GPU drivers and shader compilation
remain part of the trusted computing base. This does not guarantee that an
arbitrary shader cannot stall or crash a driver.

`examples/good/shader-switch` switches two compiled fragment passes with held
Right input. `tests/shader_gui.py` verifies the actual blue and green pixels in
an isolated display. Textures, external bindings, vertex/compute pipelines,
other rendering backends remain pending.

Hosted Linux build integrations run controlled fixtures with elevated namespace
setup privileges because the runner rejects unprivileged network namespace
configuration. The actual compilers still run inside bubblewrap with a private
network and filesystem. The application continues to fail closed when sandbox
setup is unavailable; it does not use an unrestricted build fallback.

## Bounded audio output

Games declaring `permissions.audio = true` can submit signed 16-bit mono PCM
at 48 kHz with `gamepkg::audio_submit(&samples)`. A tick may submit at most
1,600 samples in total; the device queue keeps at most 100 ms. Invalid memory
ranges, excess samples and denied permissions are rejected. Trapped ticks discard their audio. Guests receive no device handles
and cannot access microphones. The host resamples for the selected output
rate and handles volume.

The desktop includes audio by default. **Settings → Sound** offers mute and
volume controls for new play sessions. Missing or disconnected devices produce
a visible sound error while gameplay continues. Back to Library stops the
output stream. Headless CLI runs generate and check audio without playing it;
interactive terminal builds need the `audio` feature for sound output.

On Linux, desktop/audio builds require ALSA development headers
(`libasound2-dev` on Debian/Ubuntu). `examples/good/tone-pad` generates a quiet
triangle wave and changes pitch with held Up/Down. `tests/audio_game.py` tests
its signed build and PCM generation. The ignored `audio_output_null_device`
test is run explicitly with an ALSA null device to verify the actual output
callback and mute behavior without speaker playback:

```sh
echo 'pcm.!default { type null }' > /tmp/gamepkg-null-alsa.conf
ALSA_CONFIG_PATH=/tmp/gamepkg-null-alsa.conf cargo test --all-features audio_output_null_device -- --ignored --test-threads=1
```

Hosted privileged build fixtures stage toolchains under `/tmp` so namespace
root can traverse the compiler paths without access to runner home folders.

## Port feature coverage

See the [port fidelity audit and required host additions](docs/PORT_FIDELITY.md)
and [implementation tracker](docs/port-parity.toml) for remaining work and
completion criteria.

Source audit against the pinned engines and checked-in recipes (2026-10-17):
these ports are playable adaptations, not verified feature-complete replacements.
Passing startup/gameplay tests does not establish that every upstream feature or
an entire campaign works. The following gaps need implementation or explicit
product decisions before claiming full parity.

| Port | Confirmed gaps | Implementation evidence |
| --- | --- | --- |
| Cube 2 | Original startup/menu, HUD/FPS preferences and ordered desktop movement/menu key events have been restored. Physical keys now bypass egui normalization, including Pause, keypad distinctions, modifier sides and media/browser keys with SDL equivalents. Initial OS lock state and remaining uncommon-key/platform behavior still need verification. The broker now provides nonblocking TCP and UDP with unlisted peers/listeners, local addresses and broadcast under the signed on/off permission. Original registration with UDP verification, cryptographic master authentication and saved auth-key reload pass against a private master. Private Ethernet LAN broadcast discovery, original browser-row joining, wrong-key rejection and corrected-key retry are now verified. Further authentication/registration faults, live-service/platform interoperability and first-use rendering performance remain unverified. Persistence accepts selected file types, so arbitrary custom assets, movie recordings and other output formats are not equivalent to the desktop engine. Rendering inside the engine remains software OpenGL; Vulkan presents its frames. Native isolation currently requires Linux. | `src/cube2/session.rs`, `src/gui/cube2_input.rs`, `src/cube2/discovery.rs`, `engines/cube2/adapters/network.h`, `engines/cube2/adapters/persistence.h` |
| DOOM | Original WAD music, spatial stereo effects, all six original save slots and configuration persistence now work. Original desktop keyboard/mouse input, saved bindings and the quit/ENDOOM flow pass native, Wasm and desktop GUI checks. Music backend selection and broader control/platform coverage remain unfinished. Multiplayer and extra WAD/mod loading are absent. | `engines/doom/adapters/engine.c`, `engines/doom/README.md` |
| Zelda3 | Normal ROM-based SPC/DSP music and SRAM are supported. External MSU music is disabled; the desktop replay logger is removed. The adapter fixes the view to 256×224, disables extra side area, selects the default language and supplies no desktop configuration UI. Desktop enhancements, replay/save-state tools and alternate resource sets need individual support and tests. | `engines/zelda3/adapters/engine.c`, `engines/zelda3/upstream/src/config.h` |
| Arkanoo | Gravity, the speed-triggered portal and original ball-to-ball responses are restored and tested. Original world proportions and movement/contact order now match upstream reference checks. Per-ball launch timing, life recovery and victory/endless transitions also match original-code fixtures; prior item/score/level caps are removed. Brick health and original blast rules are also verified. Focused mouse movement and original Space/Escape controls are restored. Editor painting/erasing, all nine brick choices, names, save/load, clear confirmation and test return are restored. Saved custom patterns join the original twelve procedural patterns in endless play. Original ASCII pattern import/export now uses player-selected files. Gameplay cursor confinement/hiding, F11 fullscreen and pause Q-to-quit are verified in the native window and browser runtime checks. Normal size/fullscreen/maximized preferences persist through the native host. The six main menu actions and working fullscreen and sound-effects volume/mute controls are restored, with original captions, hover fills, borders, slider lettering and dimmed backdrop. Music/resolution/VSync settings, project-link/version display, external fullscreen state readback and original recorded audio remain unfinished. Original shard particles, metallic ball, glass paddle and normal-brick textures and penguin/heart-loss animation are restored. All six campaign image backgrounds, twenty procedural backgrounds and animated campaign backgrounds 7–9 match captured original OpenGL logical frames. Original speed lettering is restored and directly verified in desktop/browser players. Original editor backgrounds, their selection controls and canonical grid colors/endpoints are restored. Original editor labels, palette, Unicode pattern-name lettering and saved-pattern browser composition are restored. Complete editor/menu composition, arbitrary original window/CSS scaling and other art/effects still need work. Best score/unlocked level/gravity/effects settings persist; active runs do not. | `ports/arkanoo/CREDITS.md`, `ports/arkanoo/source/src/lib.rs` |
| 2048 | Original movement/spawn rules, merge scoring/best score, win/continue/game-over UI, restart, 128-step undo and saved board/history are supported. Tiles can merge beyond 32768. Cooperative expectimax assistance/autoplay matches pinned upstream reference decisions; one full deterministic game reaches 2048. Final signed-package delivery and broader platform checks remain open. | `ports/2048/source/src/model.rs`, `ports/2048/CREDITS.md`, `tests/game_2048.rs`, `tests/game_2048_play.rs` |

Cube 2, Zelda and DOOM now preserve stereo through their engine bridges and
shared host output. Zelda and DOOM pass native and Wasm channel-separation and
actual gameplay/save-reload tests. DOOM's mixer follows the original quadratic
separation curve; its original attract demo also verifies spatial sound.

Next priorities include remaining Cube 2 input/platform behavior and controller/touch
equivalence; DOOM music backend selection; and final 2048 package delivery/platform checks.
Arkanoo's omitted mechanics and Zelda's optional desktop enhancements require
separate feature work. Keep tests for supported paths, add a regression for each
restored feature, and test full campaigns and cross-version saves independently.

Filesystem, networking and process isolation must remain in place. Restoring a
feature means adding a scoped host operation or virtual resource, not exposing
ambient files, sockets or process execution. Threadless lock/no-device/window
stubs are not automatically lost gameplay features: inspect their caller and
host replacement before replacing them.

## Engine compatibility and save schemas

Signed manifests may declare `engine_version` as a semantic-version requirement
and `save_schema` as a positive integer. Existing manifests default to
`engine_version = "^0.1.0"` and `save_schema = 1`.

```toml
engine_version = ">=0.1.0, <0.2.0"
save_schema = 1
```

Verification and runtime loading reject unsupported engine requirements with
a message showing the required and provided versions. Engine compatibility
is also checked before installation, so an incompatible update cannot replace
a working game.

Saves are scoped to the creator, game ID and declared schema. Schema 1 retains
the existing storage path. Changing schemas opens a fresh blob and preserves
previous-schema bytes; switching back restores that namespace. No conversion
or deletion happens automatically. Keep the same schema when the game can
read its existing save layout; use a new schema for a deliberately separate
format. Automatic migration between namespaces remains pending.

Click a game’s name or icon in the library to open its page. The page shows its
engine requirement, save schema, permissions, creator fingerprint, ROM
requirement and signed lockfile inventory hash. It does not stage a download
or install anything. Creator keys normalize to their byte identity so hex
letter case cannot create different trust or save identities.


## Versioned WIT components

Set `runtime_profile = "component-v1"` at the top level of the signed manifest
with `entry_world = "game"`. Omission preserves the existing `core-v1` profile.
The authoritative contract is [`wit/game.wit`](wit/game.wit), package
`gamepkg:engine@0.1.0`, world `game`. It exports a typed `tick(input: u32,
state: u64) -> u64` and imports only `gamepkg:engine/host@0.1.0`.

Guest source still defines `game_tick` and uses the safe `gamepkg` wrappers.
The isolated compiler appends host-owned canonical ABI bindings, compiles source
into core Wasm, and encodes a validated component outside the signed archive.
No guest-supplied prebuilt component, WIT file, WASI interface, build script or
additional dependency is accepted. CLI and GUI load components through Wasmtime's
component linker with the same epoch deadline, input permission, memory
ceiling, draw/audio bounds and transactional save behavior as the core profile.
Component instances allow four internal modules and two bounded tables for the
canonical ABI; linear memory remains limited to one memory and the signed/user
ceiling. Host list copies are additional bounded host allocations, outside the
linear memory ceiling.

Assets, ROM and save reads use typed optional byte lists. Save writes and audio
use typed lists; drawing returns a boolean success value. Paths stay virtual,
ROMs remain explicitly selected, and saves remain scoped to identity/schema.
`examples/good/component-playground` is a playable component source example.
Run `tests/components_game.py` for signed build, typed list transfers, permission
denial and save rollback, and `tests/component_gui.py` for rendering and held
movement verified from actual screen pixels. The GUI test requires the same
virtual display tools as the drawing test.


## Periodic update checks

The desktop library checks installed games with signed update URLs automatically
at startup and every 24 hours while the library remains open and idle. Settings
can disable these checks or change the interval to 1–168 hours. Catalog entries
that have not been downloaded are excluded. Checks use the same HTTPS and Tor
transport, signature verification and creator/ID continuity as manual checks.

One request runs at a time. Each check downloads the complete package to verify
it, then discards that download; it never installs, builds or replaces a game.
An available version appears on the game’s page. Use **Check for updates** to
fetch it again for review and explicit installation. Failures appear in the table
and wait until the next interval rather than retrying continuously. Turning checks
off prevents new requests; a request already running may finish. No update service
runs after the desktop app closes. Restarting the app starts a fresh check cycle.

`tests/updates_gui.py` uses a local HTTPS server and isolated display to verify
that a signed update notice renders, the installed archive stays unchanged,
no early repeat request occurs, and disabled checks make no requests.


## Compiler resource and source-policy checks

Compiler discovery and version probes also use the process deadline and bounded
capture. Compiler stdout/stderr are drained after the capture limit so verbose
errors do not deadlock a build or grow the host's diagnostic buffer indefinitely.
No truncated or oversized artifact is published, and component encoding is checked
against the final 32 MiB artifact limit. A failed rebuild leaves earlier compiled
output intact. `tests/compiler_limits.py` exercises an oversized real Rust artifact;
ordinary Rust, component and Slang build integrations verify successful builds.

The game source profile rejects macro definitions and invocations (including
`assert!` and derives), renamed imports (`use ... as ...`), wildcard imports, type aliases,
closures, function-pointer types, dynamic traits, and computed call targets.
Call functions directly by name or qualified path. Qualified uppercase constant
paths remain supported. Calls through locally bound variables and passing named
functions as callback values are rejected conservatively. Ordinary imports,
modules, direct helper functions, and direct GamePKG API calls remain supported.
Reserved OS identifiers, unsafe code, foreign code, and attributes are rejected.
This syntax policy supplements compiler isolation and restricted Wasm imports;
it does not prove that source code or the host implementation are harmless.

## Game previews

Packages may include `preview.av1.webm` (AV1 in WebM) and/or `preview.ogv`
(Theora in Ogg). Previews are signed along with the game. The current profile
accepts one silent video stream, at most 16 MiB, 640×480, 30 seconds, and 900
frames, starting with a keyframe. Damaged videos, wrong codecs and oversized
previews are rejected before installation.

On Linux, install FFmpeg and bubblewrap to verify packages with previews.
Video inspection and decoding run in a separate process with no network, a
private temporary directory, a 2 GiB address-space ceiling and a 60-second
timeout. The desktop game page offers **Play preview** and **Play Theora
fallback**. Playback decodes at 320×240 and five frames per second, retains at
most 150 frames, and never starts the game. A missing preview produces an
error; preview decoding is currently supported on Linux only.

`gamepkg preview game.gamepkg` checks and decodes the preferred preview;
`--theora` selects the fallback explicitly. Run `python3 tests/previews.py`
after building release binaries to exercise real AV1/Theora videos, signing,
verification, decoding, and rejected malformed videos.

`python3 tests/previews_gui.py` uses a private Xvfb display to verify that
AV1 frames advance, the explicit Theora fallback renders, and preview playback
leaves the game unbuilt. It requires Xvfb, xdotool, ImageMagick, and Pillow.

## Shared asset store tooling

`gamepkg pool-import game.gamepkg` verifies a signed, self-contained package
and adds its assets to the library's `asset-pool` directory. Identical content
from different games occupies one blob named by its SHA-256. Imports do not
install games, approve creators, or run game code. Original packages remain
self-contained and keep their signatures.

The command prints `sha256 size virtual-path` for each asset. Use
`gamepkg pool-read <sha256> --size <bytes> -o output.bin` to verify and copy a
blob. Every read checks the exact size and content hash. Writes publish without
replacing existing blobs; a corrupted blob causes an error. Symlinked blobs and
pool folders are rejected. Individual blobs are limited to 64 MiB.

Store packages can now sign external asset references and use them through the
same scoped runtime API. Ordinary sideloaded packages continue reading their
assets directly from their own archives.

To create a store package, use:

```sh
gamepkg pack /path/to/game -o store.gamepkg --key creator.key --shared-assets
```

This adds assets to your current library's pool and replaces embedded assets
with signed `shared_assets` entries in the manifest. Each entry binds an
asset-relative path to its SHA-256 and exact size. The original source folder
is unchanged. Missing blobs are never fetched automatically; provision a
recipient's pool with `gamepkg pool-put /path/to/asset` or import the original
self-contained package with `pool-import` before installing its store variant.

Signature verification works offline without the pool. Installation, building,
and playing additionally require every referenced blob to match its signed
hash and size. Runtime reads use private session snapshots and the same 64 KiB
range limit as stored archive assets. A changed or deleted
pool blob cannot alter an active session. Logical package contents, including
shared references, remain bounded to 256 MiB. Conflicting sizes for one hash,
invalid paths, and assets declared both embedded and shared are rejected.

Run `python3 tests/shared_assets_game.py` to verify both core and WIT source
games using signed store packages, missing-blob refusal, deduplication, and
small reads from large shared assets. Portable store bundles support remote distribution as described below. Pool
garbage collection remains future work.

## Portable store bundles

A `.gamepkgstore` ZIP transports signed games and their shared assets together.
Each game retains its original creator signature, and identical shared content
is included once. The bundle contains `store.toml` (`format_version = 1`),
`packages/<signed-id>.gamepkg`, and `blobs/<sha256>.blob`. Creator approval
applies to the individual enclosed games.

```sh
gamepkg store-export first.gamepkg second.gamepkg -o games.gamepkgstore
gamepkg store-inspect games.gamepkgstore
gamepkg store-import games.gamepkgstore --trust-creators
```

Export reads shared blobs from your current library and refuses to overwrite
files. Inspect verifies every package and blob and lists creator keys without
installing anything. Import also accepts HTTPS and Tor onion URLs. First
installs require `--trust-creators`; existing games enforce the same signer and
a higher version, and an identical package is skipped. No game is built or run
during import.

All contents are staged and verified, and creator/version conflicts are checked
across the whole batch before library publication. Malformed packages,
unreferenced/missing/corrupt blobs, unexpected files, and unsupported versions
are rejected. Bounds are 10,000 entries, 64 MiB per entry, and 256 MiB per
bundle. Individual blobs and games publish atomically; an I/O failure or
concurrent library mutation can interrupt a batch after some games install.
Reimporting the same bundle safely skips completed games.

`tests/shared_assets_game.py` additionally transfers a bundle over local HTTPS
to a fresh recipient library, then builds and plays both enclosed runtime
profiles. `tests/store.rs` checks deduplication, approval, repeat imports,
conflicts, and deliberately malformed bundles.

## Dependency-free game source

Games cannot declare Cargo dependencies, including former `gamepkg-*` wrappers. The signed lockfile contains exactly the root package. Renamed, optional, Git, path, build, development, target-specific and transitive dependencies are rejected. No setting or signing key can exempt a game.

The host injects `gamepkg::` functions for scoped files, ROMs, saves, input, drawing, Slang shaders, textures, audio and achievements. Networking requests use the signed on/off permission through bounded host calls. Rust's explicit-endian conversions handle binary values without another crate. The compiler uses offline, locked Cargo in its isolated workspace.

See [developer instructions](docs/DEVELOPING_GAMES.md) for examples and migration. Host dependencies remain part of the reviewed player implementation.

## Game source modules

Games may organize source under `source/src/` using ordinary `mod name;`,
`name.rs`, `name/mod.rs`, and inline modules with separate child files. Define
`game_tick` in the root `src/lib.rs`; it can delegate to module functions. The
host appends its trusted API and export wrapper only to that root file.

Every packaged Rust file is checked, including unused files. Module declarations
must resolve to exactly one packaged file. `#[path]` and other guest attributes,
macro-generated module declarations, includes, build scripts, and non-Rust
source files are rejected. Source paths are limited to 1,024 bytes and 32
segments. Only checked files enter the private offline Cargo workspace.

`examples/good/modular-game` splits a playable game into movement, nested score
reading, and constants modules. `tests/approved_crate_game.py` builds and plays
it under both runtime profiles; `tests/modules.rs` checks missing/ambiguous
modules, path redirection, forbidden unused files, and malformed source.

Builds require a callable root `game_tick(u32, u64) -> u64`, including a
re-export from another module. Missing or incompatible entry points and
unsupported entry worlds fail before a build is published. The library checks
cached Wasm validity and the root tick signature before offering Play; older
caches without that entry point offer Build/Rebuild. The minimal, counter, and
asset-catalog examples now provide playable entry points.

`tests/build_entrypoints.py` covers successful core/WIT launches, reexports,
wrong/missing entry points, unsupported worlds, and preservation of earlier
caches after failed builds.

### Import portable stores in the desktop library

Enter a local `.gamepkgstore` path, HTTPS URL, or Tor onion URL in the Library source field, then choose **Inspect .gamepkgstore bundle**. GamePKG verifies every package signature and shared blob before showing the game versions, ROM requirements, and creator keys. Inspection does not install games. Choose **Trust listed creators and install store** to approve first installations, or **Cancel store import** to discard the review. Existing creator-key and version protections still apply. Installation uses a private verified snapshot, so changes to the original file cannot replace reviewed content. Imported games appear in the table and can be built and played.

Cargo package names use at most 64 ASCII letters, digits, underscores, or hyphens, beginning with a letter or underscore. Explicit editions must be string values `2015`, `2018`, `2021`, or `2024`; omitting the field retains Cargo's default. Invalid identity or edition metadata is rejected during package verification before it reaches the isolated compiler.

When DXIL is selected, Settings also provides a **DXC 1.9.2609 installation folder** field. The saved folder is shared by CLI and desktop builds. `GAMEPKG_DXC_ROOT` overrides the saved folder for the current launch; both paths require the same pinned library checksums. Older settings files default to an empty DXC folder.

### Game source policy

Settings explains the fixed no-dependency policy. Legacy crate approval settings are ignored and omitted on save. Dependency-bearing packages must be updated and signed again by their creators.

The desktop library and `gamepkg list` inspect installed files independently. A damaged archive, mismatched filename, or package rejected by current host policy appears as **Unavailable** with its filename and verification error; other verified games remain visible. Rejected files do not supply trusted game metadata or Play controls. Installation and store conflict checks continue to use strict verification and fail when the existing library cannot be verified.

If the library folder becomes unreadable, refreshing the desktop library clears stale rows and shows the folder error. `tests/library_gui.py` checks the actual rendered screen for a verified game beside a rejected archive, then verifies that no old rows remain after the folder becomes unreadable. The virtual-desktop test uses Tesseract to read the displayed labels.

### Build a creator source directory

```sh
gamepkg build examples/good/collect-stars -o /tmp/collect-stars.gamepkg --key creator.key
```

With `-o` and `--key`, `build` signs a temporary candidate, verifies it, and compiles its Rust and Slang source using the same isolated toolchain as installed games. Only a successful build publishes the signed source `.gamepkg`. The package retains source and assets; it does not embed the temporary Wasm or shader cache. This workflow does not install into your library. Missing signing keys, existing output files, or compilation failures leave no new output; existing files are never overwritten. Install the result and Build/Play it as usual. Without `-o`, `gamepkg build <id>` continues to compile an installed game. `pack` remains available to sign and validate source without requiring the compiler toolchain.

Creator directory-build outputs must be outside the source tree, including when their parent folder is a symlink. The destination parent must already exist. Existing files and symlinks are refused before compilation; publication still uses an atomic no-overwrite operation after successful compilation.

Existing explicitly saved shader backends remain selected. For desktop shader playback, choose WGSL for Vulkan, or choose OpenGL explicitly when using GLSL, and rebuild. New installations and Reset form to defaults use Vulkan and WGSL. OpenGL is an explicit alternative.

New builds publish a bounded shader-cache receipt tied to the signed package digest and selected backend, with checksums for each compiled output and reflection file. Playback validates the exact bytes it uses without starting the compiler. Earlier builds without a receipt retain the original compiler-based cache lookup. `tests/shader_gui.py --without-compiler` verifies actual rendering and input after removing compiler discovery; the test also rejects altered outputs and receipts for a different package or backend. This prepares the playback side for a separate isolated Windows compilation worker; The configured Windows WSL 2 connection now passes actual source build, native playback and integrity-repair tests for core and component games.

SPIR-V desktop playback accepts a single fragment entry with the bounded texture/sampler pair or no resources per cached shader. The host bounds binary input and translated output to 16 MiB, checks Slang reflection, validates SPIR-V capabilities and module structure, rejects unsupported resource bindings, and checks translated uniform/texture reflection before OpenGL setup. `tests/shader_gui.py --backend spirv` and `--backend spirv --component` verify actual shader pixels, held input, and malformed-cache rejection. Vertex/compute pipelines and other native rendering backends remain unfinished.

WGSL fragment outputs also render through the pinned Naga validation and bounded GLSL translation path. Both WGSL and SPIR-V reject unsupported bindings and non-fragment entries before graphics setup. `tests/shader_gui.py --backend wgsl` and `--backend wgsl --component` check actual pixels, input, and corrupted cache rejection. Metal and DXIL compilation remains supported, but their desktop rendering paths remain unfinished.

Desktop shader loading also requires reflection to describe exactly one entry point whose name matches the signed manifest request. Missing or mismatched entry metadata fails before playback, alongside resource-binding checks. The GUI shader tests exercise these rejection cases as well as valid rendered frames.

Fragment builds for the desktop GLSL, SPIR-V, and WGSL backends validate reflected entry points and reject unsupported resource bindings before publishing shader or game caches. SPIR-V and WGSL builds also validate their translation to the desktop renderer. A failed rebuild preserves earlier compiled artifacts.

The static site includes browser-readable [format notes](html/spec/index.html), [security model](html/guide/security.html), and [library policy](html/guide/allowlist.html). After editing the specification or security Markdown sources, run `python3 tools/render_site_docs.py` (requires Python-Markdown, packaged as `python3-markdown` on Debian/Ubuntu) and commit the generated HTML. The site serves directly from `html/` without a server-side renderer.

Games can draw RGBA sprites with `gamepkg::draw_image(x, y, width, height, pixel_width, pixel_height, &pixels)` in either runtime profile. Pixels use row-major red/green/blue/alpha bytes, with exactly four bytes per pixel. Images must be 1–512 pixels in each dimension. The host snapshots accepted pixels, checks guest memory ranges, and limits queued sprite bytes to 8 MiB per frame within the existing 1,024-command limit. Frame resets and trapped ticks discard queued images. The desktop draws sprites with nearest-neighbor sampling and alpha transparency. Shader texture access uses the separate bounded API described below. See `examples/good/sprite-playground` and `tests/sprites_gui.py`.

Each tick supplies a fresh frame. With no `draw_clear` call, the viewport uses opaque dark RGB `#10171d`. A clear color with alpha below 255 is composited against opaque black for that frame, including a fully transparent clear. Redraw every visible primitive each tick; earlier frames do not supply retained pixels. Native and browser players use the same rule.

Game rectangle/circle colors and sprite pixels use straight RGBA bytes. Native game drawing premultiplies encoded RGB bytes for source-over compositing, with a conversion separate from the app's UI styling. `tests/blending_gui.py` compares solid rectangles, nearest sprites and overlapping layers against a live SDL renderer over four backgrounds, including transparent and opaque inputs. The tested Vulkan/OpenGL desktop configurations differ by at most one 8-bit level; core/component browser samples match exactly. Transparent straight-alpha SDL interpolation, edge rasterization and complete original-game frames require separate fidelity checks.

The desktop reuses sprite textures when their pixel content is unchanged, updates the existing texture when pixels change, and releases textures when sprites disappear from the current frame or play ends. `tests/sprites_gui.py` verifies live pixel changes through held input in both runtime profiles.

Use `gamepkg::draw_asset_image("sprite.png", x, y, width, height)` to draw a PNG from a verified virtual asset path in either runtime. Embedded files and verified shared-pool snapshots work through the same archive API. Encoded PNGs are capped at 2 MiB, decoded dimensions at 512×512, and decoder allocations at 8 MiB. The host validates PNG dimensions and frame capacity before decoding the complete image. Images remain subject to the 8 MiB frame budget; missing, malformed, oversized, or traversal paths return false. These host rendering budgets are separate from guest linear-memory limits. `gamepkg::draw_asset_image_linear(path, x, y, width, height)` selects linear filtering for smooth scaling; `draw_asset_image` retains nearest-neighbor filtering. Both use the same path checks and per-draw budgets, including repeated draws of a cached image. Native and Wasm instances retain up to 32 decoded asset images within a shared 1 MiB pixel budget, using least-recently-used eviction and paths capped at 1,024 bytes. Every draw still spends its full frame budget. This session-scoped cache grants no filesystem access. The linear helper requires a host built from current source and is not available in the published 0.1.17 binaries. See `examples/good/png-sprite-playground` and `tests/sprites_gui.py --png`.

Native linear sprite sampling interpolates premultiplied encoded RGB bytes using host-owned textures, separately from UI rendering. `tests/filtering_gui.py` compares opaque gradients, nearest controls, clipped sprites and downscaled textures with SDL across static, moved and resized-source frames. Tested OpenGL output matches exactly; Vulkan and both browser profiles differ by at most one 8-bit channel level. Transparent straight-alpha SDL interpolation and complete original-game frames remain separate fidelity work.

Games can supply a texture to a supported fragment pass with `gamepkg::draw_shader_image(index, pixel_width, pixel_height, &rgba)`. The Slang pass may declare exactly one `Sampler2D gamepkgTexture` at binding 0. For SPIR-V, the host defines `GAMEPKG_SEPARATE_SAMPLER=1`: use `Texture2D<float4> gamepkgTexture` and `SamplerState gamepkgSampler` at bindings 0 and 1 in that branch, as the example demonstrates. Additional uniforms, samplers, arrays, storage buffers, and texture bindings remain rejected. Both core and WIT profiles snapshot pixels before returning, and share the 8 MiB frame image budget and 512×512 dimension limits with sprites. The renderer owns one reusable texture, uses nearest-neighbor sampling and clamp-to-edge, and replaces its contents before each pass; a plain `draw_shader` uses a transparent 1×1 texture so another pass's image cannot leak. The shader receives straight RGBA and must produce the renderer's expected output alpha convention. Textured passes support GLSL, SPIR-V, and WGSL. See `examples/good/shader-texture` and `tests/shader_gui.py --texture` (also `--component`).

Translated texture passes validate one non-arrayed, non-multisampled float 2D image at group 0/binding 0 and one non-comparison sampler at binding 1. Naga validates the complete module and its generated texture mapping before the renderer binds any pixels. Slang's combined WGSL sampler reflection occupies two descriptor slots; SPIR-V uses the explicit separate-sampler source branch. Input and output retain the 16 MiB translation bounds. Changing the SPIR-V compiler definition produces a new shader cache identity; rebuild affected games. The CI tests actual texture pixels, held-input changes, transparent fallback, and malformed caches in both runtime profiles for both translated backends. The fallback probe makes sampled RGB and alpha observable through opaque output, checks nine interior pixels, and verifies that textured drawing recovers afterward. `tests/shader_gui.py --texture --native --backend wgsl` also checks native execution; GLSL and SPIR-V use the same test options.

### GitHub project URLs

The CLI and desktop download field, catalog `download_url`, and signed manifest `update_url` also accept `https://github.com/owner/repository` or `/releases/latest`. The project must publish exactly one `.gamepkg` asset in its latest stable GitHub release. `/releases/tag/v1.2.3` selects a specific published release, including a prerelease. Projects with multiple game assets must provide a direct release asset URL. Source archives and repository code are never built just from a project link.

Public release discovery uses GitHub's anonymous API and may encounter its rate limits. Downloaded packages still undergo signature verification, first-creator approval, and update ID, version, and creator continuity checks. A GitHub account name does not establish a package creator's identity.

Run `python3 tests/github_download.py` after building the CLI to exercise GitHub release discovery with local fixtures, catalog predownload state, creator approval, signed updates, and signer/ID substitution rejection without relying on a GitHub account or API availability.

GitHub project links also work with **Import .gamepkglist** and store inspection/import. Each action selects exactly one matching `.gamepkglist` or `.gamepkgstore` release asset, independently of game assets. Catalog downloads are limited to 4 MiB before parsing; importing a project-hosted list does not download its games.

### Play history, achievements, and storage

The desktop table and `gamepkg list` show time played and achievement progress. History belongs to the signed game ID and creator key, survives updates and restarts, and is kept separately from game saves. Time measures real elapsed play sessions, including time waiting for terminal input; headless frames do not simulate extra play time. The host checkpoints every ten seconds and saves on normal exit or return to the library. A crash can lose time since the last checkpoint.

Creators can declare up to 128 achievements in the signed manifest:

```toml
[[achievements]]
id = "first_star"
title = "First star"
description = "Collect your first star."
```

Both core and WIT guests call `gamepkg::achievement_unlock(0)` with the declaration's zero-based index. Invalid indices return false. Unlocks commit only after a successful tick, remain unlocked on repeat calls, and retain their first unlock timestamp. Keep IDs stable across updates. Achievement reports come from the game; they are local progress records rather than an anti-cheat service. Collect Stars demonstrates first-star and ten-star achievements. Each game page includes an achievement list with descriptions and locked/unlocked status. The CLI also lists each achievement. Signed updates retain unlocks by stable ID even when achievements are reordered or renamed; new IDs begin locked.

Storage shows measured current package, Wasm build, identified shader caches, current save, play history, and referenced cached shared asset file bytes, plus an optional creator estimate. Shared blobs are counted once per game even when referenced under multiple asset paths. They can appear in several game totals; the library total counts each stored file once. Older Wasm builds and save schemas remain in the library total. Set `recommended_storage_bytes` under `[limits]` to estimate the total space needed after building. Estimates do not limit disk usage or guarantee the final size. The library total also includes shader caches, older builds, shared assets, saves, and play history. External compilers, user ROMs, and filesystem allocation overhead are excluded. Catalog sizes remain unknown until download; game estimates remain unknown unless the creator supplies one.

### Multiple ROMs and integrity

A signed manifest can declare up to eight player-provided files, in order:

```toml
[[roms]]
id = "program"
title = "Program ROM"
[[roms]]
id = "data"
title = "Data ROM"
```

Supply each file with repeated `--rom` options, or use **Add ROM file** in the library. Games receive read-only snapshots, indexed in declaration order, without host paths. Each file is limited to 64 MiB and the combined files to 128 MiB. Legacy `requires_rom = true` still requests one file. Guest APIs include `rom_count`, `rom_size_at`, and `rom_read_at`; existing ROM APIs read the first file.

Use `gamepkg integrity-check GAME_ID` or **Check integrity** to verify the signed archive, shared blob hashes, play history format and compiled Wasm checksum. `gamepkg integrity-fix GAME_ID` or **Fix / rebuild** rebuilds from verified source under the existing compiler isolation. Older builds without checksums need a rebuild before playback. Checksums detect accidental changes; they do not authenticate the host against someone who can modify both a build and its receipt.

Repairs never rewrite signatures, invent missing shared blobs, reset saves, or erase play history. Installation and successful checks retain a verified local recovery copy. Repair can restore a damaged archive from that copy; a damaged recovery copy is rejected. Missing recovery copies or shared blobs require verified replacement originals. Recovery copies add one archive’s size to library storage. Shader caches retain their existing validation on playback; this check currently covers the Wasm build, not every shader backend cache.

### GamePKG browser library

Run `gamepkg web-library` and open its printed local session link. No separately supplied storage package is needed. The default port is 39173; use `--port` to change it. Keeping the same port preserves the browser origin's optional cache.

GamePKG's Rust host owns packages, catalogs, settings, ROM selections, play history, achievements and saves. It rebuilds an atomic metadata snapshot at `database/library.json` from native records. The browser uses GamePKG-owned code and the standard IndexedDB API to cache metadata and icons; its cache cannot install, authorize or execute packages. Browser profile deletion never removes the native library. A cache failure still permits viewing current authenticated host records.

The loopback server requires a session token for metadata and icons and rejects foreign origins and host headers. Install, build, play and repair remain in the native desktop app or CLI. Run `python3 tests/browser_storage.py`, `python3 tests/web_library.py` and `python3 tests/home_database.py` for cache persistence, session access and native restart checks. No database server or external language runtime is required by players.


Host file snapshots reject special files such as named pipes before reading, and recheck file type and size after opening. Selected ROMs remain bounded to 64 MiB each; links explicitly selected by the player may resolve to regular ROM files. Compiled games, integrity receipts and recovery copies reject final-path symlinks. The CLI library listing reports whether the compiled game is built. `tests/special_files.py` exercises actual pipes, oversized files and normal playback with short failure deadlines.

For an end-to-end example check on Linux, configure the pinned Slang compiler and run `python3 tests/good_examples.py`. It signs, verifies, installs, builds and plays every advertised good example, then rejects every bad example for its declared reason without publishing package output. This complements the archive, runtime and bypass tests in the Rust suite.

With the compiler installed, add `--play` to `tests/web_library.py` to verify signed gameplay, achievements and measured play time in the browser cache, including restart without a valid host session token.

Run `python3 tests/progress_gui.py --browser-cache` to verify keyboard gameplay, saved achievements and browser-cache persistence. Omitting the flag runs only the desktop regression.

For an opt-in live Tor network test with a locally supplied Tor binary, run `python3 tests/tor_live.py --tor /path/to/tor`. It starts a temporary local onion service, verifies signed installation and a same-creator update, then stops Tor and removes its private test state. This requires network access and is separate from the deterministic routing test.

The internal `prepare-build-worker` command exports exactly one verified installed game and its signed shared assets into a private request store. Player saves, history, ROMs and signing keys are excluded; an existing request file is preserved. The internal `build-worker` command prepares a private compiler exchange from exactly one verified `.gamepkgstore`. It uses a temporary Linux build library and the normal offline sandbox, binds the output to the signed package digest, copies bounded compiled caches, and publishes without replacing an existing output. It does not transport player saves, history or keys, and its output is not an installable game package. The receiving `accept-build-worker` command snapshots and validates the complete response before publishing it: package identity, backend, host version, playable entry point, exact output filenames, checksums and signed shader declarations must match. Links, unexpected nested folders and excessive output are rejected. `tests/build_worker.py` verifies accepted transfers, damaged responses preserving existing builds, and playback without compiler discovery. The current source also has an experimental, explicitly configured WSL 2 connection for Windows Build/Rebuild; setup is described in `docs/INSTALL_WINDOWS.md`. Actual Windows WSL 2 source builds, core/component playback, integrity repair and native GUI keyboard-driven shader pixels passed in hosted verification using software OpenGL. Windows still needs a configured compiler and working graphics driver.

## Complete build-source parsing and persistent metadata

Before invoking a compiler, GamePKG parses every Rust file in its private build
folder, including generated core/component API bindings and unused modules. Game-owned files also pass the restricted source
policy during signature verification. Trusted host bindings have a separate policy because they implement that API.
Parsing catches syntax and policy violations; compiler isolation and Wasm
capabilities continue to enforce access limits.

The library persists under the user's home profile by default: Linux uses
`~/.local/share/gamepkg`; Windows uses `%LOCALAPPDATA%/gamepkg`.
`GAMEPKG_HOME` and the library setting can select another persistent folder.
The browser host atomically saves its authoritative metadata snapshot to
`database/library.json` inside this folder at startup and on refresh. GamePKG-owned standard browser storage remains the optional cache, populated from authenticated host records;
the home-folder snapshot contains no session authentication token or ROM bytes.

## Zelda3 ROM-required port

`ports/zelda3` ports the complete pinned [Zelda3 engine](https://github.com/snesrev/zelda3)
through a checked host API. It requires a player-selected **A Link to the Past
USA ROM** (1 MiB, optionally with a 512-byte copier header). No ROM, graphics or
music are included in the package. Resources are decoded in memory from that
selection; no extracted asset folder or data file is created. Only the game's
8 KiB SRAM save is persisted in the normal creator-scoped save folder.

Use a CLI/desktop host built from this source for playback without instruction metering. The game's combined hard
Wasm memory limit is 32 MiB, reserved across two 16 MiB instances. Engine work
uses the same frame deadline as the Rust entry.

On Ubuntu 24.04, install Clang 18, `wasi-libc`, `libclang-rt-18-dev-wasm32`,
and the normal Rust/Slang/bubblewrap build prerequisites. ROM decoding is built into
the host; building and playing Zelda3 require no Python or external decoder.
A private prepared C sysroot can be selected with `GAMEPKG_ZELDA3_SYSROOT`.
Then sign/install the source example, build `com.gamepkg.zelda3`, select your ROM
in the desktop library and click Play. The CLI supports:

```sh
gamepkg run com.gamepkg.zelda3 --rom /path/to/your/zelda3.sfc --frames 1000
```

Arrows/WASD move. Z is sword/B, X item/Y, C interact/A, V map/X,
Enter Start/pause, Tab Select/save menu, and Q/E shoulders.
Wait for the title screen before starting. File registration and normal saves
use the original game menus. See [the port guide](engines/zelda3/README.md) for
Windows setup, provenance, detailed tests and current limitations. Browser export
is currently unsupported for this builtin engine and reports that explicitly.

The Rust package entry follows the existing source restrictions. Only the
host-owned pinned C engine is compiled, inside the isolated compiler; packages
cannot supply arbitrary C code. Its Wasm has no imports, including no WASI
filesystem/network imports. The ROM decoder uses pinned host code rather than
package-supplied scripts.

## Arkanoo arcade game

`ports/arkanoo` ports gameplay from [c0m4r's Arkanoo](https://github.com/c0m4r/arkanoo).
It includes nine campaign layouts followed by endless stages, paddle spin,
three lives, multiball, long-paddle and ghost bonuses, explosive rockets, particles,
synthesized sound effects, achievements, and saved high scores/level checkpoints.
The exact upstream revision, original license and adaptation credits are included.
Rendering and synthesized effects are replacements; original music and recorded effects remain separate fidelity work.

Mouse movement, left/right arrows or A/D move the paddle. Space launches or fires;
Escape pauses or resumes. Up/W also starts, launches, fires and confirms level
transitions or restart; Down/S retains its pause/resume alias. Startup shows the
original 270-frame splash flow; Enter or a left click skips to the menu without
starting gameplay. A fresh launch starts at level one. Restart resets the run;
editor return and Resume preserve it. Up/W quick-start retains the unlocked-level
shortcut; after game over, Up restarts the campaign.
High scores and unlocked levels save when a life is lost or a level is completed.
Live state stays in the game's own isolated runtime. No ROM is required.

E opens the level editor from the title or pause screen. Select 0–8, then left
drag to paint or right drag to erase. N edits the name, S saves, C twice clears,
L opens the saved-pattern browser, and T tests the board. Test completion returns
to the editor. Saved patterns also join the endless-level pool. Patterns and
metadata share the scoped save; legacy `ARK1`/`ARK2` data migrates to `ARK3`. Original
ASCII patterns can be imported with I or exported with X using the system file
picker (or browser upload/download controls). Original art/fonts and native IME
activation remain unfinished. Names are stored as identifiers and never become host paths.

Settings includes an effects mute toggle and a volume slider, with the original
0–128 range shown as 0–100% and a 50% default. Drag the slider to change volume.
Preferences survive New Game, Restart and relaunch in the existing scoped save.
Unavailable saving and damaged saves show warnings; damaged data stays preserved.
Original music, recorded effects and the remaining menu/display controls stay
tracked in the fidelity audit.

With a current source-built CLI and GUI:

```sh
gamepkg keygen --output ./arkanoo-creator.key
gamepkg pack ports/arkanoo --output ./arkanoo.gamepkg --key ./arkanoo-creator.key
gamepkg install ./arkanoo.gamepkg --trust-creator
gamepkg build com.gamepkg.arkanoo
gamepkg-gui --play com.gamepkg.arkanoo
# Optional standalone browser package:
gamepkg web-export com.gamepkg.arkanoo --output ./arkanoo-web
python3 -m http.server 8080 --directory ./arkanoo-web
```

`cargo test --test game_arkanoo` checks campaign progression, collision response,
bonus bounds/expiry and restart. `python3 tests/arkanoo.py` performs signed
installation and isolated compilation, real Wasm audio/save/achievement checks,
native keyboard play, and Chromium gameplay in both export profiles.
Its `--skip-browser` option runs the native checks without Selenium/Chromium.
Editor, scoped ASCII pattern import/export, mouse input and portal/gravity
mechanics have original-code and actual-runtime regressions. Original menus,
music, artwork/effects, active-run saves and full platform coverage remain open;
see `ports/arkanoo/CREDITS.md` and the fidelity audit. Native window tests also
require Metacity and Python Xlib alongside Xvfb, xdotool, ImageMagick and Pillow.
These are maintainer test tools; players do not need Python.

## Open-source 2048 example

`ports/2048` adapts row sliding and merging from
[Adrien Ball's 2048-rs](https://github.com/adrienball/2048-rs), with the exact
upstream revision and adaptation details in its `CREDITS.md` and the upstream
MIT license included as a packaged asset. Arrow keys slide and merge tiles;
new tiles appear after a move changes the board. Each tile stores its exponent
in a byte, allowing merges beyond 32768. The game draws through GamePKG's checked
rectangle operations and uses no terminal, network, or filesystem interface.
`tests/game_2048.rs` verifies movement and merging; `tests/open_source_2048.py`
verifies a signed installation, isolated compilation, headless execution, and
integrity.

## Package a game for the web

Build an installed game, then export it:

```sh
gamepkg build com.gamepkg.2048
gamepkg web-export com.gamepkg.2048 --output ./2048-web
python3 -m http.server 8080 --directory ./2048-web
```

Open `http://localhost:8080`. You can upload the exported folder to static HTTPS
hosting. The GUI's **Export for web** button writes a fresh folder under the
library's `web-exports` directory and reports its location. Existing output
folders are never overwritten. The export includes a browser player, compiled
Wasm, asset data, game and player notices, and the original signed `.gamepkg`
source archive for inspection. Exported Wasm is a separate browser artifact;
it is never accepted as source inside a `.gamepkg`.

The browser player supports `core-v1` and the host-generated `component-v1`
guest ABI, with grid output, rectangles, circles, raw and PNG sprites, scoped
asset and ROM reads, bounded saves, achievements, output-only audio and mute.
Verified GLSL/SPIR-V/WGSL fragment caches supply WebGL 2 passes, including the
single bounded texture supported by the desktop host. Unsupported browser shader
features fail visibly. The 2048 example is playable in both players. ROMs are selected
by the player and never included in an export. Browser saves and play history
are scoped by game and creator; save data is also scoped by schema version.
Browser storage can be cleared or evicted and does not write desktop saves.

The guest runs in a worker with no filesystem, socket, or browser-object imports.
Export rewrites the Wasm memory maximum to enforce the manifest's hard limit,
checks the built artifact checksum, and the player checks that checksum again
before starting. A watchdog terminates a worker that does not return from a
frame. GPU work is outside Wasm time limits, and WebGL context loss stops playback. Browser execution uses a worker watchdog, and background
tab scheduling can delay a watchdog. Native compiler isolation and desktop
runtime controls remain in place. The hosting site's scripts are trusted host
code; export checksums do not authenticate a malicious hosting site.

Run `tests/web_export.py` and `tests/web_export.py --component` with the local
shader compiler and Chromium/Selenium prerequisites to verify real browser input,
scoped data, sprites, audio scheduling/mute, fragment passes, textures and the
worker watchdog in both profiles. The source scanner's guest-macro prohibition
is intentionally stricter than earlier releases: creators must repack games
that use `assert!`, derives, or other macros. Existing signed archives are not
edited by the host.

These changes are in the 0.1.5 source; published 0.1.4 archives predate web export
and the stricter source policy. Build the current source to use them. Windows
source builds require a matching 0.1.5 Linux compiler worker; older workers are
rejected before they can compile a package.

Rust inspection uses a non-recursive lexer before constructing token groups or
an AST. It limits syntax nesting to 64 groups, source tokens to 131,072 per file,
and weighted expression complexity to 1,024. Excessive operator and nesting
inputs fail cleanly instead of overflowing the parser's stack. Literal/comment
contents do not count as nesting, and the oversized-artifact fixture still
reaches the isolated compiler. `tests/parser_limits.py` covers actual CLI
rejection without publishing a partial package.

## Developer instructions

See [Developing games](docs/DEVELOPING_GAMES.md) for the creator checklist, the no-dependency policy and host API, controller and touchscreen controls, source restrictions, and build commands.

Guest Cargo dependencies are forbidden. Scoped input, file, drawing, shader, texture and audio functions come from the injected `gamepkg::` API. See the developer instructions for migration.

## DOOM port

The dependency-free DOOM recipe is in [GamePKG_Ports](https://github.com/centrexity/GamePKG_Ports/tree/main/doom). Current main supports the pinned engine through native Linux builds and optional import-free Wasm, with single-player rendering, controller/touch input, spatial stereo effects, original WAD music, all six save slots and persisted configuration. Select your own DOOM.WAD or DOOM2.WAD in the player; no WAD or extracted assets are included. These source improvements require an updated host and engine rebuild. See [engine setup and verification](engines/doom/README.md).

## Drag files into the desktop app

Drop one `.gamepkg` or `.gamepkglist` file anywhere in the library window. Packages are copied into a private snapshot, verified, and shown for creator review before installation. Choose **Cancel package review** to discard one. Lists add catalog entries as **Not downloaded**, without fetching games. Finish or cancel an existing review before dropping another file; return to Library first when playing. Unsupported files and multiple-file drops show an explanation without replacing current work.

Native and optional Wasm builds have separate controls and caches. Native C-based ports use Clang 18 and lld-18; their optional Wasm builds additionally use WASI libraries. Shader-free native games do not need Slang. See [native runtime details](docs/NATIVE_RUNTIME.md) and [HTTPS and multiplayer networking](docs/NETWORKING.md).

Play opens each game in its own process and window. The player shows only the game, with no launcher toolbar, score panel, or controls drawn over it. Resize it using your system's window controls; the game keeps its proportions. The library stays open, and closing the library leaves running games alone. Close the game window to finish the session and save play time. Import packages in the library window.

The desktop library uses high-contrast dark and light themes, artwork rows that adapt to the window width, and larger controls. Settings → Interface size adjusts text and controls from 80–160%, in addition to system display scaling.

The four port covers use MIT or CC BY 3.0 artwork with author credits and source links. Older placeholder thumbnails receive a host-provided cover without altering signed packages. Credits are available on each game page.

Packages can include a signed title, description, author, project homepage and genre labels. Game pages and the browser library display them; catalogs can provide unverified descriptions before download. See the developer instructions for metadata limits and examples.

`python3 tests/window_preferences_gui.py` verifies actual game resize, fullscreen,
maximized/fullscreen reopen, F11, normal-size restoration, OS close, library-window
isolation and damaged-record fallback at 100%/150% interface size on a private Linux desktop. It requires the
built host, Xvfb, Metacity, wmctrl, xdotool, ImageMagick, Pillow and the configured
shader compiler. Python and these desktop test tools are maintainer dependencies,
not player requirements. Window records are counted in per-game storage usage.
