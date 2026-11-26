# Game build icon plan

## Status

The GamePKG launcher now has a Windows executable icon: `build.rs` embeds
`html/assets/gamepkg.ico` in both `gamepkg.exe` and `gamepkg-gui.exe`, and the
release checks verify the PE resource. **Icons inside built games are planned,
not shipped.** This document records the reviewed design before any manifest or
native-cache format changes.

## Constraints

- A game package remains dependency-free. Game source never declares a crate,
  build script, resource compiler or operating-system call for icons.
- The icon is data from the signed source package, never an executable, script,
  symlink, device path or external download. No Nintendo asset or Nintendo code
  is bundled by GamePKG.
- GamePKG owns PNG decoding, ICO generation, temporary resource staging and
  linker arguments. The host pins their identities in the native build receipt.
- The existing strict source audit and exact-object audit stay unchanged. An
  icon cannot make unverified source playable or bypass signature checks.
- Top-level manifest additions may be optional and defaulted. Nested manifest
  tables stay strict. The `>=0.1.24` engine floor does not change.

## Phase 1 — declare and authenticate the input

1. Add an optional top-level `icon` path to the manifest, for example
   `icon = "assets/icon.png"`. Until a package declares it, builds use a
   neutral GamePKG-provided game icon; the launcher's brand icon is never
   copied into a game package.
2. Accept only a portable, package-relative PNG path: forward slashes, no
   traversal, absolute path, device name, symlink or case-collision. Bound the
   file to a small size and the decoded image to a square within explicit
   minimum/maximum dimensions.
3. Include the declared icon bytes in the signed source tree hash. A changed
   icon must change package and source-kit identity, so a stale native cache
   cannot reuse the old executable.

## Phase 2 — host-owned conversion and linking

1. Decode the signed PNG with the host's already-reviewed `image` dependency.
   Generate an ICO with 256, 128, 64, 48, 32 and 16 pixel entries using a fixed
   host resampler and deterministic ordering.
2. Generate a minimal RC script (`1 ICON "<staged ico>"`) in the host-owned
   temporary build directory. Use the same pinned Windows SDK `rc.exe` or
   MinGW `windres` discovery as the launcher build, with `GAMEPKG_RC` as the
   explicit maintainer override.
3. Compile the resource before linking. The game objects are still audited
   before any link step. The resource object is host-generated evidence and is
   never accepted as a game object or source input.
4. On non-Windows targets, keep the same signed PNG for the game window icon
   (and later the Linux `.desktop`/macOS bundle icon). No game code receives a
   window-system handle or calls the OS directly.

## Phase 3 — receipt and cache identity

Add to the signed native build receipt:

- source icon path and SHA-256;
- generated ICO SHA-256 and resource-object SHA-256;
- resource compiler identity and target architecture;
- converter/resampler version.

These values participate in the existing native cache key. A missing,
mismatched or newly invalid icon receipt forces a verified rebuild rather than
a silent reuse. The compatibility-layer ABI and shader profile are untouched.

## Phase 4 — launch and presentation

- Windows Explorer and taskbar use the embedded resource from the verified
  executable.
- The game window icon comes from the host through the same GamePKG window
  service used today; game code only supplies the signed declaration.
- If no icon is declared, use one neutral host-owned icon. If an icon is
  declared but invalid, the build fails with a specific diagnostic; Play never
  launches an executable with an unverified icon receipt.
- Library thumbnails remain catalog/signed metadata and are not promoted into
  executable resources without the declaration above.

## Phase 5 — qualification

- Unit tests: manifest defaults and strict nested-table rejection; traversal,
  symlink, oversized, non-square, non-PNG and unsigned-icon failures;
  deterministic ICO bytes and cache invalidation when the icon changes.
- Signed fixture: a package with a known 256-pixel PNG builds through the
  ordinary Build path, and `tools/windows_pe_imports.require_application_icon`
  verifies both `RT_GROUP_ICON` and `RT_ICON` in the resulting PE.
- Negative fixture: a stale receipt, modified icon, missing declared icon and
  a hand-added resource are rejected by Build and Play.
- Windows GUI proof: verify the visible game-window icon from a captured
  frame, without using any third-party or Nintendo assets.
- Re-run the full native suite, strict Clippy, app identity, release packaging
  and the Windows x86/x64 cross checks before enabling the manifest field.

## Open decisions

- Final manifest field name (`icon` versus `icon_path`) and whether a package
  may declare one icon for all platforms.
- Whether to accept a host-generated SVG source in a later phase.
- Exact neutral fallback artwork and Linux/macOS bundle destinations.
- Whether a per-game window icon should use the full PNG or only the ICO's
  256-pixel entry.
