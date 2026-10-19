# Install the Linux development preview

After installation, open **Settings → GamePKG updates** to check for a newer stable release, download it, verify it and install it into a folder you choose. **Restart into updated GamePKG** launches the new copy after you close running games. The old portable folder is kept for rollback; update your shortcuts to the new copy. Your library, ROM selections, saves and settings stay in their existing locations. The app updater requires curl, but no Python or administrator access.

This archive contains the GamePKG command-line program and desktop library. It does not include a compiler or Tor. Host binaries are provided for Linux x86-64; game packages still ship source and compile locally in an isolated process.

These builds target Ubuntu 24.04 or a compatible system with glibc 2.39 or newer. The desktop requires a working X11 or Wayland display, Vulkan and ALSA. OpenGL is available explicitly with `--renderer opengl`. The exact builder libc version and executable hashes are recorded in BUILD.json. Dependency license expressions and original notices are included in DEPENDENCIES.json and third-party-licenses/. Original resolved dependency source archives are preserved as directories in third-party-sources/, including notices embedded in source and bundled fonts.

1. Check the downloaded archive against its .sha256 file with `sha256sum -c gamepkg-0.1.17-linux-x86_64.tar.gz.sha256`.
2. Extract it into a folder you control with `tar -xzf gamepkg-0.1.17-linux-x86_64.tar.gz`.
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

Choose **Add** in the desktop library to install a signed package from a file or HTTPS URL. Review its creator fingerprint and permissions, approve installation, choose Build, then Play. Build creates a native Linux executable only. Use Build Wasm for that separate target. Native C-based ports need Clang 18 and lld-18; only the optional Wasm target needs WASI libraries. Shader-free native games do not require Slang. A game that requires a ROM needs a file you supply explicitly. Open its game page and click each ROM file field to open the system file picker. Choosing a file remembers its location for that game; cancelling keeps the previous choice. Importing a .gamepkglist only adds catalog entries; it does not download games.

To try a ready-to-install game, click **Add** in the desktop library, paste this catalog URL, and choose **Import .gamepkglist**:

```text
https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demos.gamepkglist
```

The catalog adds four games without downloading them. Zelda requires your own supported USA ROM; the three original demos do not. Click Collect Stars to open its game page, choose **Download / inspect**, review and approve its creator, then **Build** and **Play**. Those three original demos require no ROM. In Collect Stars, use the arrow keys or WASD to move toward the yellow target; collecting stars increases your score and unlocks achievements. In Shape Playground and PNG Sprite Playground, hold Left or Right to move the square or sprite. Use **Back to game** to return to its page, then **Back to library** to browse your games. The demo manifests contain signed update URLs, so **Check for updates** can verify whether a newer same-creator version is available. For offline use, download the [demo store](https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demos.gamepkgstore), then inspect and import that local file. [Demo checksums](https://github.com/centrexity/GamePKG/releases/download/v0.1.6/demo-packages.sha256) cover the packages, catalog and store.

List URLs imported through **Add** (or `import-list`) are remembered, including GitHub projects and Tor URLs. **Refresh library** updates all saved URLs and adds their latest game information without downloading games or approving creators. On **Add**, use **Update list** for one URL or **Remove list** followed by **Confirm removal** to stop following it. Removal keeps installed games and entries supplied by other lists. Failed updates keep the last good list. Re-import older list URLs once to start remembering them. Local files are imported as offline snapshots.

To create your own signed package from an included example, generate a private creator key outside the example directory:

```sh
./bin/gamepkg keygen -o /tmp/gamepkg-example.key
./bin/gamepkg pack examples/good/collect-stars -o /tmp/collect-stars.gamepkg --key /tmp/gamepkg-example.key
./bin/gamepkg install /tmp/collect-stars.gamepkg --trust-creator
./bin/gamepkg build com.gamepkg.collect-stars
./bin/gamepkg-gui --play com.gamepkg.collect-stars
```

Keep private creator keys private. Players do not need a key to install or play games; the signed manifest carries the public creator key. Checksums detect download corruption; they do not establish an independent publisher identity.

The browser library is included. Run `./bin/gamepkg web-library` and open its printed session link. It needs no separate storage package. The Rust host retains the native records; standard browser storage optionally caches library information and icons. Use the desktop or CLI for installation, builds, gameplay and repairs.

Open a game’s page and click each ROM file field to select its named file. Their locations are remembered separately across restarts; Play can launch any configured game. To repair damaged shared assets, open **Add**, enter the original local `.gamepkgstore` path, return to the library, and choose **Repair assets**, or run `./bin/gamepkg integrity-fix GAME_ID --store original.gamepkgstore`.

For separately installed WASI libraries, set **Settings → C/WASI build tools folder** to the sysroot folder (either `/usr` or a folder containing `usr/include/wasm32-wasi` and its matching libraries). Use **Check C/WASI build tools**, then **Save settings**. Leave it empty for system packages. Engine-specific environment overrides still take precedence. The Windows compiler uses the settings saved inside its Linux environment.

## Zelda3 optional engine

For the 0.1.6 Zelda3 port, Ubuntu 24.04 additionally needs Clang 18,
wasi-libc and libclang-rt-18-dev-wasm32. No Python or external ROM decoder is
needed for building or playing Zelda3. Build ports/zelda3 as a signed package
and select your supported USA ROM in the library. No extracted assets are written.
See engines/zelda3/README.md for the complete setup and verification details.

## DOOM source engine

Version 0.1.14 includes the pinned DOOM Wasm engine. The Linux compiler needs the same Clang 18 and WASI tools described above. See [DOOM engine setup](../engines/doom/README.md) and the [port instructions](https://github.com/centrexity/GamePKG_Ports/blob/main/DOOM.md). Select your own DOOM.WAD/DOOM2.WAD; no WAD is included. Windows compilation uses the existing opt-in WSL 2 exchange; native Windows DOOM playback has not been tested locally.

See [native runtime setup](NATIVE_RUNTIME.md) for target-specific caches and isolation, and [networking](NETWORKING.md) for separately declared HTTPS and multiplayer permissions.

## Original-data native engine

GamePKG 0.1.15 includes the reviewed Cube 2 engine. Its native build needs GCC/G++, Make, SDL2, SDL2_image, SDL2_mixer, zlib and OpenGL development libraries. On Ubuntu 24.04:

```sh
sudo apt install build-essential libsdl2-dev libsdl2-image-dev libsdl2-mixer-dev zlib1g-dev libgl1-mesa-dev libgl1-mesa-dri
```

Import the [ports catalog](https://github.com/centrexity/GamePKG_Ports), download and inspect the signed selector, then Build. Choose an original Sauerbraten installation containing `data` and `packages` on its game page. The game reads these directories in place; no original media is included in the package. Set the memory ceiling to at least 2048 MiB in Settings. Software rendering may need a 500 ms tick deadline. Play opens a separate game-only window, with Vulkan presentation by default. This engine currently runs on Linux; it is not a Windows native engine. Python is not needed.

See [engine setup, controls, saves and multiplayer](../engines/cube2/README.md). Networking stays off until you explicitly choose a join/hosting address or a master for the server browser. Local/private addresses need their separate opt-in.
