# Native Windows gameplay validation

These checks concern the reviewed Takedawn profile. Its prepared game functions
compile to native Rust code; the pinned original platform runtime remains
C/C++. Players do not need Python, C2Rust or WSL. The owned supported ISO is read
in place. Maintainer tools and private test saves are separate from player
requirements and the user's library.

## Verified routes

The production runtime builder was exercised with the actual standalone Windows
tools under Wine, including an extended SDK path containing spaces. The resulting
executable has SHA-256
`6f858f72c89957abf6b5c0b38afbddae6ca7ed0dfffbf63980db4de34eb8efeb`
and retains all 24,032 native exports. All game bodies were compiled from the
prepared Rust units.

Reviewed original menu/pad inputs and the original debug AI were used for these
World Tour checks. No medal, score, unlock or race result was injected.

| Route | Observed result | Persistence check |
| --- | --- | --- |
| First Silver Lake race | First of six, two laps, 3:35.55; best lap 1:46.78. Gold, three new events, three awards and 2,095 points. | Original autosave succeeded. A fresh process showed 1/55 USA and 1/173 overall gold medals, with the new Downtown Road Rage event available. |
| Newly unlocked Downtown Road Rage 1 | Natural Totaled result with three takedowns. Silver, two new events, one award and 1,800 more points. | Original autosave succeeded. Another fresh process showed the silver event medal and the new face-off events. |

After each cold load, the active profile and metadata matched their saved bytes
exactly. The pre-medal reference stayed unchanged. All three private probes
finished successfully, and none reached an unresolved upstream game-function
stub. Probe termination after menu inspection is separate from a normal game
shutdown; normal settings restart and shutdown have their own checks.

The earlier original-driver Single Event Road Rage attempts remained in play
past the driver's 900-second result deadline. They remain recorded as incomplete
natural-finish checks. They are not counted as successes or silently replaced
by the World Tour results above.

## App and platform checks

Native host validation and Windows packaging pass for app revision `1a0eacc` in
[run 40240219657](https://github.com/centrexity/GamePKG/actions/runs/40240219657).
The exact Windows ZIP has SHA-256
`bdc729ee130017ca99483d53b07f8f071b4f4cbfe3a335c260014d53716bd0a7`.
Its isolated Wine checks pass CLI/GUI versions and embedded audio, temporary
signed-package installation, default Vulkan rendering, resize/close and actual
OLE catalog import with persistence and no package download.

A subsequent import audit found that this app artifact still requires an
unbundled `VCRUNTIME140.dll`. Its functional checks do not qualify it as a
standalone package. The release builder now requests the static MSVC runtime;
packaging checks inspect normal and delayed PE imports, and Wine qualification
disables the separate MSVC runtime DLLs.

The replacement app at `d38f284` passed native Windows packaging in
[run 40251483831](https://github.com/centrexity/GamePKG/actions/runs/40251483831).
Its ZIP has SHA-256
`0b5705fef780c04067267e8ce42323b12aac539b6b45a0d26e2be86beb41e5b7`.
Both app binaries pass the import check. The exact package passes all of the
isolated Wine CLI/GUI, Vulkan, resize/close and OLE catalog import checks above
with the external MSVC runtime DLLs disabled. The game executable and its SDL
companion pass the external compiler-runtime import check independently. The
separate 32-bit packaging path now requests the same static-runtime policy; its
native result is pending.

All automated audio output is dummy/muted. Rendered captures and non-silent
audio captures do not establish audible fidelity or physical controller access.
Wine gameplay also does not establish original-game GPU/audio/controller access
inside an actual Windows AppContainer.

The full signed Windows install, native Build and integrity repair gate passed in
[run 40229229052](https://github.com/centrexity/GamePKG/actions/runs/40229229052).
It compiled all 486 prepared Rust units with native optimizations, produced a
ready-to-play library entry, rejected a damaged SDL companion, then repaired it
while preserving every compiled object hash and modification time. No owned
media or gameplay was used in that build gate.

Native frame/hang policy, broader campaign coverage and actual Windows hardware
qualification remain separate requirements. No stable release is
qualified by these routes alone.

See the [Windows build notes](../engines/takedawn/rust-port/WINDOWS_BUILD.md)
and [native process ownership notes](../engines/takedawn/windows-runner/README.md)
for the SDK, sandbox, committed-memory budget and restart mechanisms.
