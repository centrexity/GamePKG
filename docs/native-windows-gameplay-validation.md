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
exactly. The pre-medal reference stayed unchanged. These three race/Road Rage
probes finished successfully and reached no unresolved upstream game-function
stub. Probe termination after menu inspection is separate from a normal game
shutdown; normal settings restart and shutdown have their own checks.

The newly unlocked Waterfront Face-Off also reaches its original loss result:
one lap, second of two, 2:17.01, 165 points and no takedowns. Original Continue,
autosave and points screens return to the map without unlocking its reward. A
fresh process retains 4,060 global points, six takedowns and two completed events;
the Face-Off reward remains locked. The profile and metadata load byte-for-byte
unchanged. This cold probe exits zero. The initial event probe's outer wrapper
terminated with status 143 during its intentional stop after the menu return;
that harness failure remains recorded separately. The cold probe isolates game
and driver process sessions and stops cleanly. Neither reached an unresolved stub.

The earlier original-driver Single Event Road Rage attempts remained in play
past the driver's 900-second result deadline. They remain recorded as incomplete
natural-finish checks. They are not counted as successes or silently replaced
by the World Tour results above.

## Online scope

These routes qualify offline gameplay. The
[pinned upstream plan](https://github.com/xenophobentx/takedawn/blob/5a00fc10f9548a189e9043c5a55975863ad49175/README.md#whats-next)
lists System Link/LAN, XLink Kai and Insignia as future work. The signed profile
currently disables networking. This integration does not claim those online
modes, and local multiplayer/physical controller behavior is not qualified by
the single-player probes. The separate GamePKG networking APIs do not establish
that this original game's online services work.

## App and platform checks

Native Windows and macOS host validation pass for app revision `b2b43fd`,
including the history contention fix, in
[run 40254964579](https://github.com/centrexity/GamePKG/actions/runs/40254964579).
The exact Windows ZIP has SHA-256
`2c77d459c8fd8073cc1aa7676deb86287ddd5f85760f96cf536f40279e34e644`.
Its isolated Wine checks pass CLI/GUI versions and embedded audio, temporary
signed-package installation, default Vulkan rendering, resize/close and actual
OLE catalog import with persistence and no package download.

The release builder selects the static MSVC runtime. Both app binaries pass
normal and delayed PE import checks, and the exact ZIP passes the Wine checks
with external MSVC runtime DLLs disabled. The game executable and its SDL
companion pass the external compiler-runtime import check independently. This
avoids the unbundled runtime requirement found in an earlier app artifact.

Windows32, Linux32 and Android32 checks pass in
[run 40257219069](https://github.com/centrexity/GamePKG/actions/runs/40257219069).
The Windows32 gate also executes both actual app entries after relocation,
without a display or external decoder files. The x86-64 game itself still
requires a 64-bit system.

The unpublished hardware preview uses this exact native app and the verified
signed v13 recipe, with a separate library and settings. Every extracted app
file matches the native ZIP. The source changes through `08fd7f0` affect tests
and documentation only; production app source is unchanged from `b2b43fd`.

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
