# Install the Linux development preview

This archive contains the GamePKG command-line program and desktop library. It does not include a compiler or Tor. Host binaries are provided for Linux x86-64; game packages still ship source and compile locally in an isolated process.

These builds target Ubuntu 24.04 or a compatible system with glibc 2.39 or newer. The desktop requires a working X11 or Wayland display, OpenGL, and ALSA. The exact builder libc version and executable hashes are recorded in BUILD.json. Dependency license expressions and original notices are included in DEPENDENCIES.json and third-party-licenses/. Original resolved dependency source archives are preserved as directories in third-party-sources/, including notices embedded in source and bundled fonts.

1. Check the downloaded archive against its .sha256 file with `sha256sum -c gamepkg-0.1.2-linux-x86_64.tar.gz.sha256`.
2. Extract it into a folder you control with `tar -xzf gamepkg-0.1.2-linux-x86_64.tar.gz`.
3. Enter the extracted folder. Run `./bin/gamepkg --help` for the CLI or `./bin/gamepkg-gui` for the desktop.

On Ubuntu 24.04, install the runtime tools needed for isolated builds, downloads, previews, and audio:

```sh
sudo apt install bubblewrap coreutils util-linux curl ffmpeg libasound2t64
```

Install Rust with rustup using its official instructions at https://rustup.rs, then add the game compilation target:

```sh
rustup target add wasm32-unknown-unknown
python3 tools/install_slang.py "$HOME/.local/share/gamepkg/slang-2026.19"
```

Enter that Slang folder in the desktop Settings page, or set GAMEPKG_SLANG_ROOT to it for the CLI. The installer verifies the pinned compiler archive before extraction. Namespace support is required for isolated builds; a denied sandbox stops compilation. GLSL is the default desktop shader backend. DXIL compilation additionally requires the pinned DXC installer; native Metal/DXIL playback remains unfinished.

Install a signed package from a file or HTTPS URL in the desktop library. Review its creator fingerprint and permissions, approve installation, choose Build, then Play. A game that requires a ROM needs a file you supply explicitly. Importing a .gamepkglist only adds catalog entries; it does not download games.

To try a ready-to-install game, paste this catalog URL into the desktop library and choose **Import .gamepkglist**:

```text
https://github.com/centrexity/GamePKG/releases/download/v0.1.2/demos.gamepkglist
```

The catalog adds three games without downloading them. Choose **Download / inspect** for Collect Stars, review and approve its creator, then **Build** and **Play**. None of these demos requires a ROM. For offline use, download the [demo store](https://github.com/centrexity/GamePKG/releases/download/v0.1.2/demos.gamepkgstore), then inspect and import that local file. [Demo checksums](https://github.com/centrexity/GamePKG/releases/download/v0.1.2/demo-packages.sha256) cover the packages, catalog and store.

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

For games with multiple ROMs, select the game and supply every named file in manifest order. To repair damaged shared assets, enter the original local `.gamepkgstore` path in the desktop package field and choose **Repair assets**, or run `./bin/gamepkg integrity-fix GAME_ID --store original.gamepkgstore`.
