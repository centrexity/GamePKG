# Install the Linux development preview

This archive contains the GamePKG command-line program and desktop library. It does not include a compiler or Tor. Host binaries are provided for Linux x86-64; game packages still ship source and compile locally in an isolated process.

These builds target Ubuntu 24.04 or a compatible system with glibc 2.39 or newer. The desktop requires a working X11 or Wayland display, Vulkan and ALSA. OpenGL is available explicitly with `--renderer opengl`. The exact builder libc version and executable hashes are recorded in BUILD.json. Dependency license expressions and original notices are included in DEPENDENCIES.json and third-party-licenses/. Original resolved dependency source archives are preserved as directories in third-party-sources/, including notices embedded in source and bundled fonts.

1. Check the downloaded archive against its .sha256 file with `sha256sum -c gamepkg-0.1.13-linux-x86_64.tar.gz.sha256`.
2. Extract it into a folder you control with `tar -xzf gamepkg-0.1.13-linux-x86_64.tar.gz`.
3. Enter the extracted folder. Run `./bin/gamepkg --help` for the CLI or `./bin/gamepkg-gui` for the desktop.

On Ubuntu 24.04, install the runtime tools needed for isolated builds, downloads, previews, and audio:

```sh
sudo apt install bubblewrap coreutils util-linux curl ffmpeg libasound2t64 libudev1 libvulkan1 mesa-vulkan-drivers xdg-desktop-portal xdg-desktop-portal-gtk zenity
# For C-based source ports:
sudo apt install clang-18 lld-18 wasi-libc libclang-rt-18-dev-wasm32
```

Install Rust with rustup using its official instructions at https://rustup.rs, then add the game compilation target:

```sh
rustup target add wasm32-unknown-unknown
curl --fail --location --output slang-2026.19.tar.gz https://github.com/shader-slang/slang/releases/download/v2026.19/slang-2026.19-linux-x86_64-glibc-2.27.tar.gz
echo '5899bd40c3d1ee60eadd1d1b37acc4e88ae65cc25ac61651534d55b71a85fe75  slang-2026.19.tar.gz' | sha256sum --check
mkdir -p "$HOME/.local/share/gamepkg"
mkdir "$HOME/.local/share/gamepkg/slang-2026.19"
tar -xzf slang-2026.19.tar.gz -C "$HOME/.local/share/gamepkg/slang-2026.19"
```

After the checksum passes, extract into the new Slang folder as shown above. No Python installer is needed. Enter that folder in the desktop Settings page, or set GAMEPKG_SLANG_ROOT to it for the CLI. Namespace support is required for isolated builds; a denied sandbox stops compilation. Vulkan with WGSL is the default desktop shader path. DXIL compilation additionally requires the pinned DXC installer; native Metal/DXIL playback remains unfinished.

Install a signed package from a file or HTTPS URL in the desktop library. Review its creator fingerprint and permissions, approve installation, choose Build, then Play. Build creates a native Linux executable only. Use Build Wasm for that separate target. Native C-based ports need Clang 18 and lld-18; only the optional Wasm target needs WASI libraries. Shader-free native games do not require Slang. A game that requires a ROM needs a file you supply explicitly. Open its game page and click each ROM file field to open the system file picker. Choosing a file remembers its location for that game; cancelling keeps the previous choice. Importing a .gamepkglist only adds catalog entries; it does not download games.

To try a ready-to-install game, paste this catalog URL into the desktop library and choose **Import .gamepkglist**:

```text
https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demos.gamepkglist
```

The catalog adds four games without downloading them. Zelda requires your own supported USA ROM; the three original demos do not. Click Collect Stars to open its game page, choose **Download / inspect**, review and approve its creator, then **Build** and **Play**. Those three original demos require no ROM. In Collect Stars, use the arrow keys or WASD to move toward the yellow target; collecting stars increases your score and unlocks achievements. In Shape Playground and PNG Sprite Playground, hold Left or Right to move the square or sprite. Use **Back to game** to return to its page, then **Back to library** to browse your games. The demo manifests contain signed update URLs, so **Check for updates** can verify whether a newer same-creator version is available. For offline use, download the [demo store](https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demos.gamepkgstore), then inspect and import that local file. [Demo checksums](https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demo-packages.sha256) cover the packages, catalog and store.

To create your own signed package from an included example, generate a private creator key outside the example directory:

```sh
./bin/gamepkg keygen -o /tmp/gamepkg-example.key
./bin/gamepkg pack examples/good/collect-stars -o /tmp/collect-stars.gamepkg --key /tmp/gamepkg-example.key
./bin/gamepkg install /tmp/collect-stars.gamepkg --trust-creator
./bin/gamepkg build com.gamepkg.collect-stars
./bin/gamepkg-gui --play com.gamepkg.collect-stars
```

Keep private creator keys private. Players do not need a key to install or play games; the signed manifest carries the public creator key. Checksums detect download corruption; they do not establish an independent publisher identity.

The browser library is included in this preview. Run `./bin/gamepkg web-library --indexeddb-package /path/to/Hindsight4/Compiler/packages/Database/IndexedDB` and open its printed session link. Hindsight’s package is supplied separately. The browser stores library information and icons in IndexedDB; use the desktop or CLI for installation, builds, gameplay and repairs.

Open a game’s page and click each ROM file field to select its named file. Their locations are remembered separately across restarts; Play can launch any configured game. To repair damaged shared assets, enter the original local `.gamepkgstore` path in the desktop package field and choose **Repair assets**, or run `./bin/gamepkg integrity-fix GAME_ID --store original.gamepkgstore`.

For separately installed WASI libraries, set **Settings → C/WASI build tools folder** to the sysroot folder (either `/usr` or a folder containing `usr/include/wasm32-wasi` and its matching libraries). Use **Check C/WASI build tools**, then **Save settings**. Leave it empty for system packages. Engine-specific environment overrides still take precedence. The Windows compiler uses the settings saved inside its Linux environment.

## Zelda3 optional engine

For the 0.1.6 Zelda3 port, Ubuntu 24.04 additionally needs Clang 18,
wasi-libc and libclang-rt-18-dev-wasm32. No Python or external ROM decoder is
needed for building or playing Zelda3. Build ports/zelda3 as a signed package
and select your supported USA ROM in the library. No extracted assets are written.
See engines/zelda3/README.md for the complete setup and verification details.

## DOOM source engine

Version 0.1.13 includes the pinned DOOM Wasm engine. The Linux compiler needs the same Clang 18 and WASI tools described above. See [DOOM engine setup](../engines/doom/README.md) and the [port instructions](https://github.com/centrexity/GamePKG_Ports/blob/main/DOOM.md). Select your own DOOM.WAD/DOOM2.WAD; no WAD is included. Windows compilation uses the existing opt-in WSL 2 exchange; native Windows DOOM playback has not been tested locally.

See [native runtime setup](NATIVE_RUNTIME.md) for target-specific caches and isolation, and [networking](NETWORKING.md) for separately declared HTTPS and multiplayer permissions.
