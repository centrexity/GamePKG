# Exclusive app publication checks

This maintainer-only harness imports the actual updater publication module and
runs without building the app, graphics stack or game packages:

```sh
cargo test --locked --manifest-path tools/publication-tests/Cargo.toml
cargo clippy --locked --manifest-path tools/publication-tests/Cargo.toml --all-targets -- -D warnings
```

Linux uses `renameat2(RENAME_NOREPLACE)`. Darwin uses `renamex_np(RENAME_EXCL)`;
see [Apple's kernel implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_syscalls.c).
Windows uses [MoveFileExW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-movefileexw)
with zero flags, excluding replacement, cross-volume copying and reboot actions.
Windows input is checked for NUL and normalized without resolving the destination
leaf, then qualified for extended disk/UNC paths. Unsupported publication APIs
fail explicitly; no check-then-overwrite fallback is used. The app's public
updater platform selection remains Linux/Windows.

The same tests run in the root Cargo suite. They check existing files, empty and
nonempty directories, a concurrent empty-destination creation race, Unicode and
invalid paths; Windows also checks paths longer than 260 characters and Unix
checks dangling destination links. The independent hosted workflow runs Linux,
Windows and macOS, plus native 32-bit Windows execution. Local Windows GNU builds
can also compile this file directly as a standalone test harness; Wine results
are compatibility evidence, not a substitute for native Windows/macOS runs.
No release archives, user folders or real update downloads are used.

The native workflow at source revision `0710239` passed all 16 cases across Linux,
macOS, Windows64 and Windows32, including the long-path and creation-race checks:
[verified run](https://github.com/centrexity/GamePKG/actions/runs/39186503336).
This result covers the publication primitive, not full updater downloads or game
feature/platform parity.
