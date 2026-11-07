# Native Windows gameplay validation

These checks concern the reviewed Takedawn profile. Its prepared game functions
compile to native Rust code; the pinned original platform runtime remains
C/C++. Players do not need Python, C2Rust or WSL. The owned supported ISO is read
in place. Maintainer tools and private test saves are separate from player
requirements and the user's library.

## Current v17 campaign and preview checks

The signed v17/0.1.6 recipe uses app 0.1.19 and source kit
`8ae26ce68862dffde427315fc335e2e6effa1d26ecd9ad02778499c4d478a7d0`.
Its normal Linux native build retains the local CPU/O3 optimizations. The
unchanged executable earns Gold in Waterfront Face-Off: one lap, first of two,
136.100 seconds. Original rewards report 450 points, one takedown, a new car and
two new events. Successful autosave and a fresh process retain 4/173 events,
2,615 points and three takedowns. A subsequent fresh launch can select the earned
Modified Compact for the newly available Compact Grand Prix.

That Grand Prix completes all three original races with the same executable:

| Route | Place | Laps | Time | Grand Prix points |
| --- | --- | --- | --- | --- |
| `US/C3_V2` | 3/6 | 2 | 243.650 s | 3 |
| `US/P1_V1` | 5/6 | 1 | 211.333 s | 1 |
| `US/C1_V1` | 2/6 | 2 | 285.883 s | 4 |

The original final standings award Bronze with eight points. Normal Continue
actions traverse the 4,525-point/six-takedown summary, successful autosave,
Muscle Series film and three car presentations, achievement notifications and
the campaign map. Driver Details then shows 5/173 events, 7,140 points and nine
takedowns. An independent cold process retains those exact totals, the new
Downtown race and unlocked Muscle Types 1, 2 and 3. All five earned UDATA/TDATA
files remain byte-identical during the cold check; no partition images are
copied. Both drivers exit zero and pass the actual-Vulkan and executed-path
checks. The 39 championship and nine cold-load captures decode and match their
recorded hashes. The private receipts are
`v17-grandprix-progression-qualification.json` and
`v17-faceoff-progression-qualification.json`.

These runs use the existing maintainer helper to put player one's car under
the original game's AI control. It changes control state, but does not inject
finishes, times, scores, medals or unlocks. Audio stays on the dummy driver and
the owned ISO stays read-only in place. This does not establish every campaign
event, manual-player physics, physical controllers or audio equivalence, and
does not relabel this Linux Grand Prix run as physical Windows coverage.

The current physical Windows app preview has separate normal Build/Play/history,
remembered-ISO, integrity and desktop checks. Its exact preview launcher also
opens the prepared library and game page with the saved ISO and Play enabled.
A reported Needs ROM screen had only the older 0.1.18 process open; the current
0.1.19 preview was not running. Opening the current preview and its scoped
library restores Ready without replacing the ISO selection. The current game
page was left open for hands-on controls/audio testing, which remains unqualified.

The prepared Rust game inputs retain all 13 unresolved-reference guards.
Additional original-byte audits identify coefficient tables, device/update
path strings and a 16-byte HMAC input among apparent code entries. Their scoped
reader/control-flow checks and corruption controls do not prove whole-program
indirect-entry exclusion; no guard is removed or replaced with game behavior.

## GPU report lookup correction in the v18 development kit

A warning-log review of the completed v17 Grand Prix found that the GPU
executor's periodic report sorts the live unhandled-method table in place.
The constant-time method-to-slot lookup then points at different entries, so
later counts and last parameters can be assigned to the wrong method. This is
a diagnostic defect; the rendering dispatch does not use that lookup table.
The aggregate counter and a successful Vulkan route are not proof that every
reported graphics method is implemented.

The new pinned-source adapter ranks a snapshot instead. Both native host build
attachments and the source-kit preparer include it; unknown upstream revisions
and in-place edits are rejected. Its regression compiles the actual production
counter and complete report block: the original returns the expected failing
status after a report, while the adapted source passes repeated top-ten/all
reports with aligned, unaligned and out-of-range methods. Linux address/undefined
sanitizers and physical Windows 32-bit/64-bit fixtures pass. The pinned upstream
source hash is independently checked against the CI commit. Native CI now runs
the regression too; its hosted result is not yet claimed.

Maintainers can run `tools/check_takedawn_gpu_report.py --source /path/to/pinned/xboxrecomp --sanitize`
on Linux. Windows MSVC uses `--cc cl --compiler-kind msvc` without the sanitizer
flag. These tools are not player dependencies.

The unreleased 0.1.7 recipe and app 0.1.20 now select v18, archive SHA-256
`d94c9bdb3a5e49733ebbe78c573e56174b00ca7ffb83ffbfd9fd0fa091c0905b`.
The complete archive inventory verifies: only the two host build attachments
and the added report adapter differ from v17. All 494 prepared inputs and 487
Rust units remain byte-identical. Normal signed Linux and physical Windows
Build/integrity tests pass while preserving all 487 same-machine cached objects'
hashes and timestamps. This is verified reuse, not a new cold Rust compilation.

Both new native runtimes pass selected-ISO and remembered-ISO launches, increasing
play history and post-play integrity. The new Linux Vulkan runtime also finishes
an original three-lap Single Event Race, fifth of six in 394.183 seconds with a
124.883-second best lap. Normal Continue/autosave returns to Select Car. The
private profile changes; original reference saves and the built binary do not.
Actual-Vulkan and executed-path gates pass. Fifty-three production reports have
unique ranked entries and nondecreasing observed method counts; the native
regression separately establishes slot stability. No rendering or speedup claim
is inferred from that diagnostic check.

The 0.1.20 Windows library renders Ready after a 1.711-second window observation.
Its exact GUI native Play child opens a separate game window, closes normally
and adds history. These are startup and lifecycle checks of the new Windows
runtime; the complete physical Windows race/reload proof remains tied to v17.
The user's v17 preview, saved ROM selection and launcher remain unchanged.
Further graphics-method classification, manual controls/audio and broader game
fidelity remain open.

The 0.1.20 Linux library's actual Play button also opens the new native runtime
in a separate SDL window on a private test display. Ordinary keyboard input
loads an unchanged earned profile and its Team Crush / Trailer Trash $26,737
record. Normal window close reaps the player, increases history and refreshes
the game page's Ready/Play state and duration without a manual library refresh.
The library closes normally, all five save files remain unchanged, and the
actual-Vulkan/executed-path checks pass. A private null-only audio sink keeps
this test silent; it does not qualify physical controls, audible output or GPU
performance.

An original-byte check also identifies the observed `NtFsControlFile(0x90020)`
cleanup call in routine `001D3054`. The import resolves to ordinal 200 and the
local Windows SDK identifies this control as `FSCTL_DISMOUNT_VOLUME`. The original
caller ignores its return and I/O status, closes the same guest handle, then
branches on an earlier saved status. Three corruption controls reject changed
arguments, imports or result handling. The bridge still returns
`STATUS_NOT_SUPPORTED`; this explains the scoped cleanup path without claiming
virtual-volume semantics are implemented or forwarding controls to host volumes.

## Retained v16 unresolved-reference audit

The prepared v16 source still retains 13 unresolved-function guards. An
ISO-in-place static audit now explains three apparent references without adding
replacement game behavior:

| Apparent unresolved target | Original data mistaken for a branch |
| --- | --- |
| `000817D9` | Generated parent `000817BC` starts inside the five-entry jump table at `000817B0`. The apparent branch at `000817C0` decodes the fifth table pointer. |
| `00184C19` | Generated parent `00184BCA` extends into the two-entry jump table at `00184BCC`. The apparent branch at that address decodes its first pointer. Six selector bytes at `00184BD4` select only those two entries. |
| `00320C2A` | The apparent branch at `00320C1D` decodes the last pointer in the eight-entry table at `00320C01`, after the original code in `003205A8`. Its apparent destination is an existing instruction within `00320C21`. |

The original functions at `000816D0`, `00184AD0` and `003205A8` have unsigned bounds checks
that dominate their dispatches. All table destinations are decoded instruction
boundaries within their respective code spans. The audit follows 52, 70 and 441
reachable instructions in those function-local control-flow graphs, including
their original returns. None of the apparent branches belongs to those code spans.
The first two apparent destinations fall one byte inside existing six-byte `mov`
instructions in neighboring real functions. The third is a local instruction
within its neighboring function, rather than that function's entry.

The owned image is checked against the supported SHA-256 before reading these
spans. The private receipt retains code/table hashes and the current prepared
source hashes. Mutation controls reject a table target into data, an out-of-range
selector, substitution of a signed guard and changing the third dispatch bound.
No media is extracted to disk.

This classifies the three table-derived references; it does not establish every
possible dynamic entry. All 13 guards, prepared functions and shipping build
objects remain unchanged. The other targets, broader reachability, full gameplay and physical platforms
still require qualification; the string review below adds a separate scope.

### String-derived references

Four further guarded targets (`0036B7E2`, `0036B7F2`, `00772C57` and
`00772C5E`) have five recorded branch instructions decoded from original
strings: `\Device\MU_0`, `\Device\MU_%x` and `update.xbe`. All five
instruction byte spans fall within those exact ASCII literals. Original
instructions in three known functions also push the string addresses as
arguments; those references agree with the current prepared Rust source.

The earlier private audit's `non_code_section` conclusion for the first two
targets is insufficient. Their original `.rdata` section has flags `6`, which
include executable permission. The section label does not establish that the
bytes cannot execute. The new classification concerns the actual string-derived
branch bytes, with original source hashes and section flags retained.

Two changed-string controls fail, as do cross-section and truncated-byte reads.
All reads stay in memory. Together with the three table cases above, this
explains recorded data-derived references to seven guarded targets. All 13
guards remain; other targets and possible dynamic entries still require review.
The crypto review below adds a separate scope. No native game rebuild or
player-library change is involved.

### Crypto-input-derived reference

The apparent call at `00772C3A` to `E5A14D74` decodes five bytes within an
original 16-byte crypto input at `00772C30..00772C40`. On the reviewed path
where the second caller argument is zero, `0031B561` passes that span as
`Data1` at two call sites. The original thunk `001D702A` reads import slot
`0036B920`, whose encoded ordinal is 340; the pinned runtime binds that ordinal
to `XcHMAC`. The shared second call site can receive a different input on the
other caller path, so this is a path-specific data-use proof.

The review checks the actual push arguments, input length, caller branch and
nonvolatile register contract against the original instructions and pinned
bridge. Current prepared source and runtime files match the kit inventory.
Mutations of the input length, caller branch, import ordinal and apparent-call
bytes are rejected. No original input bytes are written to disk.

This explains recorded data-derived references to eight guarded targets when
combined with the table/string reviews. All 13 guards remain. The other five
targets, possible dynamic entries, complete crypto behavior and full gameplay
and platform qualification remain open.

### Six-entry switch-table follow-up

The recorded branch at `0006EA35` to `E9DAEA40` decodes bytes within the
original six-entry table at `0006EA30..0006EA48`. Requalification against the
current v16 kit follows all 64 reachable instructions from `0006E950`,
including the six switch destinations, original returns and the original
external tail jump to `000AB850`. The unsigned comparison against five
and its guard dominate the table dispatch. Every table destination is an
instruction boundary before the data span; no reviewed entry path reaches
the apparent branch.

The supported image hash, current prepared Rust/table-dispatch source hashes,
and existing v16 executable identity match. Controls changing a table entry
into data, increasing the bound, substituting a signed guard or changing the
external tail destination are rejected. The audit assumes unchanged original
table bytes and ordinary returning external calls; it does not establish
all possible dynamic entries or behavior within those external functions.

Together these reviews explain recorded data-derived references to nine of
the 13 guarded targets. The four remaining targets, global reachability,
complete crypto behavior, full gameplay and physical-platform qualification
remain open. All guards and compiled objects are preserved; the ISO is read
in place without writing media to disk.

### HMAC-key-derived reference

The apparent call at `00772C5A` to `80370B8D` lies within the original
16-byte key at `00772C58..00772C68`. The known original function `0031C82D`
passes that address and length to the HMAC thunk at call site `0031C8B8`,
after its nonvolatile-settings query succeeds. The argument pushes and
length initialization dominate the reviewed call. `Data1` is a separate
16-byte span reached through the pointer stored at `0036B7CC`; no second
data span is supplied.

Both thunks resolve to the expected imports: nonvolatile-settings ordinal 24
and HMAC ordinal 340. The length register's preservation is checked against
the explicit stdcall contract and pinned bridge/runtime source. Current
prepared Rust source hashes and the existing v16 executable identity match.
Controls changing the key length, key pointer, HMAC ordinal or apparent-call
opcode are rejected. No original key bytes are written to disk.

This explains recorded data-derived references to ten guarded targets in
combination with the preceding reviews. The three WMA targets remain open;
the coefficient-pattern candidate is not yet classified because its decoder
read path remains unverified. All 13 guards remain. These checks establish
the reviewed key-use path, without claiming a crypto-result oracle, global
dynamic-entry safety, full gameplay or physical-platform completion.

### WMA word-table-derived references

The apparent conditional branches at `00310AC0` and `00310AC2` to
`00310A76` and `00310A78` are two words within the original decoder table
at `003103B8..00310D70`. The known initializer `00309A89` installs that
table pointer in the second argument object's field at offset `0x14` on
its reviewed selection path. The original caller `003098B5` passes that
field to the word-table reader `0030C891`.

The reader requests 23 input bits. Its three lookup loops take five and
three two-bit choices, followed by up to six one-bit choices. A traversal
matching the original instructions covers all possible choices: every
lookup stays inside the 1,244-word table and reaches a marked terminal
entry. It covers 889 terminal entries, with encoded lengths up to 20 bits;
both apparent branch words are actual table reads. This table uses no
extended-symbol reads, whose address calculation is also checked.

Four controls reject a child offset outside the table, an extended-symbol
read beyond its end, a changed reader loop bound and an unaligned initializer
pointer. Prepared Rust source hashes and the existing v16 executable identity
match. All reads remain in memory, and all game guards remain unchanged.

Combined with the preceding reviews, this explains recorded data-derived
references to 12 guarded targets. `003152C1` remains open: its exact
coefficient-pattern match still needs a qualified read path. The word-table
proof assumes the selected pointer and original table bytes; it does not
qualify every interprocedural state, the bitstream callback, decoded-audio
equivalence, possible dynamic code entries or complete gameplay/platform
behavior.

### Converted WMA lookup function behavior

A private native fixture compiles the unchanged generated Rust body of
`sub_0030C891` against register-storage, callback and function-lookup test
adapters. The original table is read from the supported ISO and supplied only
through memory/stdin. An independent prefix-code reference constructs a full
20-bit lookup map and rejects conflicting codes or incomplete coverage.

Both native Linux ELF64 and ELF32 runs compare all 8,388,608 requested 23-bit
patterns. Each also checks 4,096 full-width cases with optional remainder
outputs and successful callback statuses, plus nine negative-callback cases.
The symbol, consumed-bit count, remainder, callback arguments, stack cleanup
and saved EBX/ESI/EDI values match. Failed callbacks preserve the output
sentinels. The original table remains unchanged, and both widths produce the
same result fingerprint.

The fixture uses the exact manifest-verified generated function body, without
rebuilding the 487 game units or changing the game executable. Its test ABI
adapters are separate from the shipping runtime. This qualifies the converted
lookup body for the reviewed table; actual stream callbacks, runtime TLS,
complete WMA decoding, PCM/audio equivalence and physical platforms remain
separate checks. No original table bytes are written to disk.

## Prepared v15 multiplayer input follow-up

The unreleased 0.1.4 recipe selects the v15 kit, SHA-256
`7f56741fa8f0a56d3e718af1f772bae9200514b6293c2d218e56cde700123c8b`.
It changes only the staged native controller policy and its CMake attachment;
all 487 prepared Rust units and 24,035 exports are unchanged from v14.
Older v14 recipes remain admitted for identity/save continuity.

An actual v14 split-screen run reproduces a missing PS2-control gate: both
players are in play under the original multiplayer module, but their physical
bottom buttons remain Xbox A instead of the accelerator. The new read-only gate
follows the title's own controller assignments and device ports, then checks
each player's phase, intro and result state. It does not assume that the first
physical controller owns player one. Shared pause/exit transitions stop driving
controls; one player's results do not change the other player's active controls.
Keyboard/touch input keeps the existing source exclusions and slot ownership.

The complete staged input source passes 213 assignment/state/handover checks
under Address, UndefinedBehavior and Leak sanitizers at both native pointer
widths, with a compile-time 32-bit requirement for the ELF32 CI mode. All 15
glyph-family checks still pass. A native reference build compiles the changed
input object and relinks hash-verified existing runtime/Rust objects; it retains
all 24,035 exports. It is not a fresh rebuild of every source file.
Actual split-screen checks pass PS2 accelerator mapping for both players,
held-button suppression when pause opens, and mapping restoration after Resume.
Both Xbox and Nintendo face layouts pass with player one pausing. A separate
completed run verifies player two owning Pause, suppressing both held buttons
and restoring both accelerators after Resume. Original Vulkan and unsupported-path
gates pass, with dummy audio and unchanged private references. A first helper
assertion incorrectly excluded controller reads on the exact pause-transition
frame; its failure and same-frame traces are preserved before the corrected run.

The same incremental reference executable also passes the pinned original
single-player Xbox Race and PS2-layout Crash assertions. Race retains countdown,
acceleration, held-button pause/Resume and Quit/Yes. Crash retains countdown,
accelerator response, impact/aftertouch/crashbreaker, results handover and
Continue to the front end. These two runs pass original Vulkan/unsupported-path
gates with dummy audio and read-only ISO access; they remain incremental
reference evidence rather than normal signed v15 Build/Play qualification.

Both app entries build and six real source-kit handoff/rejection tests pass.
The normal signed Linux v15 package now passes an unseeded optimized native Build:
all 487 units compile from an empty game-object cache in approximately 62 minutes,
and all 24,035 prepared exports remain. The initial private preflight omitted the
native tools folder and stopped before any unit compiled; configuring the
existing documented folder resolves that setup error.

Normal integrity, wrong-disc rejection without saved selection/play time,
selected-ISO Play, a second launch with remembered ISO and increasing play history
all pass. The ISO stays in place. Deliberately damaging the private executable
fails integrity; source repair and final integrity succeed while preserving all
487 compiled-unit hashes and modification times, the signed package, settings,
ROM selection, play history and player data.

The exact repaired executable (SHA-256
`621988980facdace69fc4dbf1e2b6de4043b442dd8d910854f09df38002b30df`)
then passes five actual native control routes: the pinned original Xbox Race and
PS2 Crash checks, plus split-screen Xbox-face/P1 Pause, Nintendo-face/P1 Pause and
Xbox-face/P2 Pause. The latter three verify both accelerators, held-button
suppression while paused and restored mapping after Resume. Viewed resumed
screens show both players moving. All runs pass strict executed-path and actual
Vulkan gates with dummy audio, read-only ISO access and unchanged private
reference saves. Virtual pads stand in for physical controllers. These are
scoped control routes, not completed multiplayer races or physical device tests.

The full Windows v15 signed Build/repair [workflow 40612591810](https://github.com/centrexity/GamePKG/actions/runs/40612591810) now passes on actual Windows at `b40bbbe`. Its cold optimized Build completes in about 4 hours 26 minutes on the CI machine. The library recognizes the native build automatically, deliberate SDL-companion damage fails integrity, and normal repair restores it in about 112 seconds while preserving all 487 Rust object hashes/timestamps and the signed package. The verification-only artifact passes its published SHA-256 and archive CRCs; no original media is used and no gameplay is claimed. The executable is 59,915,264 bytes, SHA-256 `fd95c9d028d6d5eb17ae810efa4e0530ee70fd1766a4f6608fddb8cf69caa12e`. This qualifies the v15 build/repair path, not the newer generic Windows source compiler work or a stable release. Physical
Windows original-game graphics/audio/controllers, mixed split-screen
result/driving prompt presentation, natural multiplayer completion, higher medals
and full campaigns remain unqualified. The immutable v14 hardware preview stays
unchanged and does not contain the v15 input fix.


The new private v15b hardware-test bundle pins the verified `081a5b1` Windows
app and ports `d8672b5`, with a separately signed 0.1.4/v15 recipe. All 18,595
app files match the verified native archive byte for byte. Its native game
builder/engine/ports inputs match the successful `b40bbbe` full-game build;
the app additionally retains explicit Steam settings and browser startup fixes.
It deliberately does not include the newer, unqualified generic Windows
source-compiler work. The bundle contains no ISO, original-media extraction,
private creator seed or prebuilt CPU-specific game.

`GamePKG-Windows-Preview-v15b-081a5b1.zip` is 268,594,298 bytes, SHA-256
`776039653964b13ae7ce0c4059ad21b5b500afade02551f67638f94591f82b68`.
Its launcher uses absolute paths beside the script for a separate preview
library and settings; default-off watchdog and disabled automatic updates remain.
A private, network-disabled Wine desktop verifies the actual packaged Windows
CLI signature/import and original CMD launcher, visible v15 library with Needs
ROM, explicit settings overriding invalid inherited settings and normal Alt+F4
close. Automated audio is dummy/null with no host audio device exposed. It does
not build or play the original game during this launcher check. The first v15
candidate and failed private launcher/forced-window-destruction checks remain
preserved; the v15b archive is a distinct immutable file, and old v14 ZIPs are
untouched. Physical Windows original-game GPU, sound and controller testing,
complete multiplayer/campaign qualification and stable publication remain open.

## Verified v14 baseline

The verified 0.1.3 recipe selects v14, with source-kit SHA-256
`7cfd18d26b8a6e51005f75d7cedc1c01925237574bc579df5027b0167413a986`.
It requires the 0.1.18 development app. All 24,003 prepared Rust game functions
are retained across 487 units and 24,035 native exports. The older 0.1.17 stable
app does not include this native engine.

The normal signed Linux package passes a cold native Build, wrong-disc rejection,
ISO-in-place Play, a second launch with its remembered ISO, play-history saving
and integrity checks. Intentional private-cache corruption is detected and
repaired; all 487 unit-object hashes and modification times, signed package,
settings, ROM selections and player data are preserved. The exact repaired
executable passes original Race and PS2-layout Crash controls.

Private tests of that same repaired executable also cover:

| Route | Observed result | Fresh-process check |
| --- | --- | --- |
| First World Tour race, Silver Lake | Second of six, two laps, 222.383 seconds; best lap 105.467 seconds. Original results, reward/save flow and a new Downtown Road Rage event. | Profile/metadata load with identical hashes; the new Road Rage1 event is selectable. This is not a gold-medal result. |
| Newly unlocked Downtown Road Rage1 | Natural Out of time loss with zero takedowns, zero new points and no medal. Original Continue, successful autosave and return to Select Location. | Profile/metadata load unchanged; Driver Details retains one completed event, 1,135 points and one takedown from the preceding race. |

The title's own debug AI drives these tests with reviewed menu/pad inputs. No
score, medal, unlock, event time limit or save result is injected. All four processes use
dummy audio and pass the actual-Vulkan and unsupported-path gates. These are
scoped campaign checks, not verification of all 173 events.

A separate current-v14 run disables the title's debug AI and uses normal steering,
accelerator and boost inputs. It earns the original Downtown Road Rage1 Bronze
medal with one event takedown, 400 points and two newly unlocked events. The
original Totaled result, Continue, autosave, points breakdown and map all run.
A fresh process shows the bronze icon beside Road Rage1, 2/173 completed events,
1,535 global points and two total takedowns. The earned profile and metadata
remain byte-for-byte unchanged through cold loading. Both processes exit zero
with actual-Vulkan and strict unsupported-path gates enabled and dummy audio.
The driver uses the original pause/resume menus between reviewed actions, then
holds normal acceleration continuously to the natural finish. It writes no game
memory, score, event timer, medal, unlock or result. This qualifies that medal and
persistence path; it does not establish an entirely uninterrupted real-time event,
higher medal tiers, all campaign events or physical controller behavior.

A separate unpaused headless check now drives the entire event with normal
steering, accelerator and boost inputs and debug AI disabled. It reaches the
original Totaled/Bronze result with one takedown, 400 points and two new events.
The actual controller-read log shows all five Continue presses reaching the
game. Original reward and successful autosave screens run, and saved bytes
change. A fresh process loads that exact earned save and displays 2/173 events,
1,535 global points, two takedowns and the Road Rage1 bronze icon. Its active
profile, metadata and earned reference remain byte-for-byte identical after
loading. Both owned processes exit zero under the actual-Vulkan and strict
unsupported-path gates, using dummy audio and the ISO in place. There are no
pause transitions after initial unpaused state, game-memory/result writes or
debug-AI activation. This qualifies an unpaused headless event/save path; physical
controllers, Windows hardware and exact wall-clock pacing remain separate.

An earlier unpaused check earns Bronze with two takedowns but stops before
Continue; it does not qualify that result's persistence. Private save-driver
errors are retained separately: a long held input postpones menu commands, and
parse-time input scheduling was incorrectly treated as controller delivery.
The latter attempt finishes with Bronze but never delivers Continue and leaves
the save unchanged; its zero process exit is not a passing save qualification.
The corrected driver bounds queued holds by the observed game clock, waits for
actual controller reads and rejects unchanged earned saves. A negative/positive
check rejects the undelivered trace and recognizes the earlier real delivery.
These are maintainer-driver corrections, not production deadline changes.

The complete v14 Windows archive also builds with actual standalone Windows
Rust under Wine. Its linked executable has SHA-256
`4467bfd5ae1b91565a99cd06792d6695bc2d61e22626f62f4e6b5dbd66ae6540`.
It passes original profile create/reload, unchanged saves, stereo capture,
Race/PS2-layout Crash controls, keyboard menu navigation, 1024×768 resize and
normal close. It finishes an original three-lap Race, fifth of six in 345.483
seconds, with a best lap of 108.517 seconds. Traps, actual-Vulkan and unsupported
path gates are enabled. Wine does not establish physical Windows game behavior.

Actual Windows/macOS host validation passes at app `f87f383` in
[run 40581542080](https://github.com/centrexity/GamePKG/actions/runs/40581542080).
Both hosts pass their test and strict lint jobs. Windows also passes managed
standalone SDK setup, isolated playback fixtures, optional WSL profile checks
and verified ZIP packaging. These host fixtures do not establish original-game
SDL/Vulkan/audio/controller behavior on physical Windows hardware.
The focused [confined Windows window gate](https://github.com/centrexity/GamePKG/actions/runs/40581541946)
also passes at that app revision. It uses production ownership with synthetic
media and no WSL, including the default-off watchdog launch.

The earlier private Windows hardware bundle uses app `f87f383`, ports `e230359`
and the signed v14/0.1.3 recipe with separate preview library/settings. All 18,593
app files match its verified native Windows ZIP, and all 178 bundled port PNGs
match the reviewed source. It includes the native Windows storage and window
color fixes and default-off watchdog. It contains no ISO, private signing key or
prebuilt CPU-specific game. Its complete ZIP SHA-256 is
`19f9a53d9f84c468c003f71f82ef5174c02f7d2cf171951224b49d1f3936c676`.
This is a private hardware-test bundle, not a stable release qualification.
The full v14 native
Windows signed Build/repair gate passes on actual Windows in
[run 40446319498](https://github.com/centrexity/GamePKG/actions/runs/40446319498):
all 487 units build with native optimization, the library recognizes the built
game automatically, integrity rejects an intentionally damaged SDL companion,
and repair restores it while preserving every unit hash/modification time and
the signed installed package. The build takes 7,517 seconds and repair 74 seconds
on that runner. Compiler children can discover only the verified SDK and Windows
System32 tools. This gate uses no ROM and does not qualify original-game playback;
physical Windows game checks remain required before stable promotion.

## Historical v13 Windows routes

The earlier v13 production runtime builder was exercised with the actual standalone Windows
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

## Reached untranslated paths

The stricter executed-path audit found that the older v13 runtime reaches
`wbinvd` at `0x00353DB5`, `cli` at `0x00367C86` and `sti` at `0x00367C97` in
both profile-create and reload logs. The upstream translator currently reports
these instructions and treats them as no-ops. Historical route and save/audio
results above remain observed outcomes; they do not prove complete instruction
semantics. That v13 executable fails the stronger path gate.

The shared maintainer gate now rejects every reached untranslated instruction,
rather than checking only the two MXCSR instruction names, and still rejects
reached unresolved function stubs. Window/restart checks use the same rule;
the native restart probe explicitly enables upstream stub logging. Its fixtures
cover arbitrary future instruction names, split log chunks, earlier failed
restart phases and missing logs. The v14 candidate implements the reached cache and interrupt operations through
native Rust helpers and the original kernel's host dispatch gate, retaining the
original following-instruction interrupt shadow. Both real-kernel Linux and
standalone Windows conformance checks pass, including rejected masking-guard
mutations. Its complete standalone Windows Rust build retains all 24,035 exports
across 487 units; native runtime linking also succeeds (executable SHA-256
`4467bfd5ae1b91565a99cd06792d6695bc2d61e22626f62f4e6b5dbd66ae6540`).
The Linux candidate passes profile create/reload, full three-lap Race, original
Race/Crash control checks, and a 15,000-flip Time Attack session with normal
Quit/Yes/Continue back to the front end. The Windows candidate also passes profile create/reload, unchanged save hashes,
stereo capture, original Race and PS2-layout Crash control checks, keyboard profile
navigation, a 1024×768 window resize and normal close under Wine. These use an
independent headless GPU desktop and retain actual-Vulkan and unsupported-path
assertions. The first resize attempt was constrained by a 720-pixel test desktop;
the unchanged assertion passes on the taller desktop. With instruction traps
enabled, the Windows candidate also completes the original three-lap race:
fifth of six, 345.483 seconds, best lap 108.517 seconds, normal results flow and
strict path/Vulkan gates. Normal signed Linux Build/Play/repair and the updated
private preview and full v14 Windows signed Build/repair are now verified as
described above; physical Windows qualification remains required. The rejection is not waived because
the game reaches a menu or a result screen.

## Player handling of unsupported paths

The native player enables the runtime's immediate untranslated-instruction trap
and unresolved-stub logging. Linux and Windows also audit the original output
stream: a reached unsupported instruction or unresolved function stops the owned
session and produces a launch error. Failure state survives verbose output,
fragmented reads and managed restarts. Final output is audited even when the
process exits zero or the host requests a stop. These checks are separate from
the optional, default-off frame-stall watchdog. They report missing execution;
they do not implement unqualified game paths. Older admitted source packages can
therefore require a package update before they play.

## Offline two-player coverage

A bounded Windows/Wine probe reaches the original multiplayer Race with two
virtual pads, separate car selection, split cameras and both HUDs. Player one
reaches 39 mph while player two stays at zero; a separate player-two throttle
input produces 38 mph while player one is stopped. Both then receive concurrent
throttle inputs and move through the track. Debug AI is off. The original main
already defaults to two pads; no production default change is needed.

The reviewed-input probe eventually exceeds its 900-second overall deadline
while still in the race. It does not qualify a natural multiplayer race finish.
A separate follow-up opens and resumes the player-two pause menu, then uses the
original Quit/Yes flow to return to multiplayer setup and exits zero. Neither
probe reaches an unresolved function.

A controlled sequence confirms both players' pause/resume, including transfer
back to player one. Original read-only state traces match in the native C
reference, Linux Rust build and Windows Rust build. The Windows run also returns
through player one's original Quit/Yes flow and exits zero. An additional Windows
run with control logging disabled captures a fresh `Paused - Player 1` screen
and exits zero. The earlier screenshot-only missing-pause finding is withdrawn;
it did not establish a delivered-input failure. Physical pads, a complete
multiplayer race and actual Windows hardware still need qualification.

A separate concurrent comparison fails Vulkan buffer initialization and falls
back to CPU rendering. It is retained as invalid graphics qualification. The
maintainer gameplay gate now requires the actual Vulkan executor's confirmation
and rejects CPU fallback; a device-created message alone is insufficient.
The save/audio gate applies the same check independently to its create and reload
processes, before publishing a passing result. This includes native Linux runs.
Regression fixtures cover incomplete initialization, mixed restart outcomes and
markers split across log chunks. Earlier qualified route logs also pass this
stronger check.

## Online scope

These routes qualify offline gameplay. The
[pinned upstream plan](https://github.com/xenophobentx/takedawn/blob/5a00fc10f9548a189e9043c5a55975863ad49175/README.md#whats-next)
lists System Link/LAN, XLink Kai and Insignia as future work. The signed profile
currently disables networking. This integration does not claim those online
modes. The bounded two-player observations above do not qualify physical
controllers or complete multiplayer play. The separate GamePKG networking APIs do not establish
that this original game's online services work.

## Earlier app and platform checks

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

The superseded hardware preview used this exact native app and the verified
signed v13 recipe, with a separate library and settings. Every extracted app
file matched the native ZIP. That preview pinned production app `b2b43fd`;
the subsequent `c766be1` app change adds the optional frame-stall diagnostic
watchdog described below. That older preview predates the setting; the current
v14 bundle described above includes it.

All automated audio output is dummy/muted or routed to a verified private
null-only sink. Rendered captures and non-silent
audio captures do not establish audible fidelity or physical controller access.
A fresh serialized Windows/Wine save/audio repeat completes the 45-second
capture and unchanged profile reload, but reports at least 55 SDL queue
underruns during the introductory sequence. A preceding unchanged run reports
none. The capture gate therefore proves complete mixed output and save behavior,
not consistently smooth playback; audio timing remains an open qualification.

Two later unchanged Windows/Wine runs record the private null sink through
WASAPI and compare its actual output with the game's capture. Each complete
45-second stream matches every PCM byte at one fixed recording offset, with
no queue-empty reports during either compared create process. The second
reload logs one queue-empty event; its output has no separate WAV reference
and is not qualified by the create comparison. These successful repeats do not
resolve earlier intermittent failures or prove hardware audio.
The maintainer tool `tools/check_takedawn_pcm_output.py` checks the complete
48000 Hz stereo s16le stream, using the measured preroll in `--offset-frames`.
Its fixtures reject inserted silence, dropped/reordered frames, changed
channels, truncation, invalid formats and incorrect offsets.


An independent audio-path control queues a known 45-second stream completely
before starting playback, with no game code, renderer or per-frame audio
producer involved. The Linux SDL 2.30.0/PulseAudio path reproduces all 2,160,000
stereo frames byte-for-byte (PCM SHA-256
`8042adebd51f89b51fa3aa65b4e2a21d93c089adfe0d6615658e6af253c6f0eb`).
Both Windows SDL 2.32.10/Wine/WASAPI control recordings contain discontinuities.
In the second, the initial reference anchor is present and the first exact
comparison fails at reference frame 489,568; later sampled anchors are displaced
by 1,393 frames. The first also differs at startup and later within the stream.
Only a private null sink and its monitor are loaded; the active Windows stream
has unity volume, 48 kHz stereo and the server's copy resampler. All control
processes finish successfully and consume their prequeued stream in about
45 seconds. The captured output, rather than successful process exit or queue
consumption, is what exposes the mismatch.

These controls establish that this Windows-through-Wine recording path can fail
without a game-side audio producer. They do not isolate Wine, SDL, host scheduling
or the recorder as the cause: the Linux and Windows SDL versions and transport
formats differ. They also do not resolve the game's separately observed
queue-empty reports or qualify real Windows hardware. A failed Wine recording
alone is insufficient evidence for changing the game's buffers or audio code.

Wine gameplay also does not establish original-game GPU/audio/controller access
inside an actual Windows AppContainer.

The full signed Windows install, native Build and integrity repair gate passed in
[run 40229229052](https://github.com/centrexity/GamePKG/actions/runs/40229229052).
It compiled all 486 prepared Rust units with native optimizations, produced a
ready-to-play library entry, rejected a damaged SDL companion, then repaired it
while preserving every compiled object hash and modification time. No owned
media or gameplay was used in that build gate.

The native frame-stall watchdog is optional and explicitly off by default for
new and existing settings. In the reviewed runtime it watches actual flips and
writes diagnostic reports without terminating the game. Settings accepts 5–600
seconds when enabled; startup allows at least 60 seconds. Saving, cancelling and
disabling the option pass the real desktop settings checks. Actual Windows
host checks pass in [run 40433416799](https://github.com/centrexity/GamePKG/actions/runs/40433416799),
including saved-setting restart and default-off launch. This host fixture does
not qualify the original game's physical Windows graphics or controls. Broader
campaign coverage and actual Windows hardware qualification remain open. No stable release is qualified
by these routes alone.

## Private GPU display for maintainer Wine checks

On Linux, the window and keyboard probes can run on a separate headless
Weston/Xwayland desktop, without activating windows on the user's desktop:

```sh
python3 tools/with_private_gpu_display.py --vulkan-probe -- \
  python3 tools/check_takedawn_windows_window.py \
  /private/build/burnout3-gamepkg-rust.exe /path/to/owned.iso /private/reference-saves
```

The helper needs Weston, Xwayland, Metacity, D-Bus, xauth, xprop and xdotool;
`--vulkan-probe` also needs vulkaninfo. These are maintainer tools, not Windows
player dependencies. It creates its own short Wayland socket path, authenticated
X11 display, window manager and compositor data directories. Rootful Xwayland
allows real XTest key events and focus checks without a physical Wayland input
seat. Child test commands retain their existing memory scopes and audio controls;
the game probes explicitly use dummy audio. The helper cleans up only its own
process groups and records display logs. A successful Vulkan probe or display
startup does not replace the game's actual-executor, input, save, audio or
unsupported-path assertions, and does not qualify physical Windows hardware.

See the [Windows build notes](../engines/takedawn/rust-port/WINDOWS_BUILD.md)
and [native process ownership notes](../engines/takedawn/windows-runner/README.md)
for the SDK, sandbox, committed-memory budget and restart mechanisms.

A current v14 Linux completion probe reaches the original multiplayer module
`003A939C`, with both player phases entering `2` (in play), under strict path
checks and actual Vulkan. It does not establish a race finish: the pinned
`src/game/autopilot.c` helper only permits the single-player module `003A9ACC`,
so its player-one AI never engages here. The owned probe was stopped once this
invalid driver assumption was identified; its reference profile remained
unchanged. Enabling that debug flag is not evidence of multiplayer driving.
A completion check still needs a reviewed ordinary-input driver; production
AI guards, scores and results must not be patched to manufacture qualification.

## Ordinary Windows source compiler diagnostic

This is separate from the passing signed v15 prepared-game Build/repair and the
immutable v15b hardware preview. Ordinary Windows source-game support remained disabled during these diagnostics;
the subsequent normal-loader integration is recorded below.

[Paired Windows controls](https://github.com/centrexity/GamePKG/actions/runs/40651190995)
at `7441a6c` show the pinned LLD version probe succeeds outside confinement with
the same cleared environment, but fails inside AppContainer with either regular
file or private-pipe output. Output writer read-attribute rights do not fix it.
[Focused path checks](https://github.com/centrexity/GamePKG/actions/runs/40651991680)
at `0c5c292` show `GetLongPathNameW` returns access denied for both ordinary and
extended paths, including the probe's own executable. LLVM's Windows startup
uses this lookup before processing linker inputs. An ordinary image path also
fails; no relaxation of confinement or compiler fallback has been added.
The [ancestor diagnostic](https://github.com/centrexity/GamePKG/actions/runs/40652577335)
at `ba013a0` returns the same access-denied result for every ancestor through
`C:\`. Those diagnostics alone did not qualify a compiler fix; no ancestor ACLs were widened.

The optional watchdog remains off by default. Its exact settings persistence and
validation regression passes locally; both above Windows runs pass the separate
real-window/default-off watchdog test before reaching the compiler failure.

### Qualified standalone compiler and shared SDK

[Actual Windows run 40656721386](https://github.com/centrexity/GamePKG/actions/runs/40656721386)
passes both PE32 and PE64 jobs at `b818f303020d9aed22fdde3c7fc7e5fdece41dc3`.
The selected linker is GNU Binutils 2.42 `ld.exe` already included in the pinned
Rust 1.89.0 toolchain. Rust produces its optimized fat-LTO static library; the
host starts the native linker separately. Both stages retain AppContainer/job
bounds, private streams, one absolute deadline and the existing directory grants.
There is no unconfined fallback, additional download or ancestor ACL expansion.

Each actual Windows width passes compiler startup, native PE identity, exact
binary-message exchange and rejection of an ungranted host-file include. The
signed source package passes native Build and generated SDK execution, including
asset reads, drawing requests, high 32 bits of u64 state, scoped save data retained
across fresh processes, and damaged-build rejection. The shared fixture also
passes locally under Linux's normal build/worker isolation and is invoked
explicitly in Linux CI. The separate Win32 window/restart/default-off watchdog
fixture passes in the same Windows run.

The verification-only archives match their published SHA-256 and CRCs: 32-bit
artifact 12802221666 is `24494f318c282fb7cd87332f17de3e817e32beece6d06b199b87cf66ba92e830`;
64-bit artifact 12801504559 is `600e60da6095de684c5a1d831de6248ba2a0896205579455aed6015e9d2a8abf`.
They contain only test logs. Earlier missing-thumbnail/license and incorrect
asset/verification fixture failures remain recorded. The final fixture follows
production's signature verification before asset access. No owned media or
original-game gameplay is exercised here.

This qualified the standalone compiler/shared SDK contract at `b818f30`; its
ordinary Windows source-game entry points were still disabled at that head.
Subsequent shader and normal-loader integration is recorded below. Physical
original-game checks remain open. The
immutable v15b hardware preview and earlier whole-game Build/repair are separate.

### Native Windows source-game and shader entry points

The [shader/compiler workflow](https://github.com/centrexity/GamePKG/actions/runs/40662676099)
passes at `5df761a`, and the [normal-loader integration](https://github.com/centrexity/GamePKG/actions/runs/40670512933)
passes at `0f9c0a0`. Ordinary Rust source packages now use native Windows x86/x64
Build/Play entry points in the development source. The normal loader discovers
the native artifact, verifies its signed package, restores scoped saved bytes,
records actual elapsed play time and rejects damaged native/shader artifacts.
Its explicit-settings test seam shares the production loading body and avoids
process-global test configuration changes. Reviewed engines retain their own
platform support; this does not enable every C-based engine on Windows.

Slang 2026.19 is installed from its pinned official Windows archive without any
vendor installer or user Python requirement. All 4,942 files are checked on reuse;
selected external Slang folders must match that complete reviewed payload. Native
shader compilation uses the shared bounded AppContainer/job runner, private work
files and explicit cleared environment. Linux retains its existing bubblewrap
compiler boundary. Signed SPIR-V/WGSL/GLSL native builds pass reflection and
translation validation, normal game loading and shader requests. Damaged shader
outputs fail playback, and an ungranted host include is denied while the same
file compiles in the deliberate unconfined test control. This is compilation and
runtime-request evidence, not a native GPU pixel/rendering qualification.

The [mixed-bitness workflow](https://github.com/centrexity/GamePKG/actions/runs/40672606863)
passes at `3442e33ec00553849d89973a8c1a1338f518b552`. It includes both PE32/PE64
source regressions, all three native shader backends through normal game loading
at both app/game widths, and the actual Win32 window/restart/default-off watchdog
fixture. The 32-bit app launches the verified 64-bit Slang tool on AMD64 Windows;
OS capability is checked independently of the app's pointer width. Its native
Rust game/compiler remain 32-bit. Genuinely 32-bit Windows can use shader-free
source builds, but still needs a suitable shader compiler before shader support
can be qualified there. These hosted 32-bit processes run on a 64-bit OS.

At that head, Windows DXIL setup, physical GPU/input/audio and full original-game
fidelity stayed open. The initial private audit verified Microsoft's DXC 1.9.2609
Windows ZIP and its 33 files, before production installer integration.
A headless Wine prototype compiles DXIL and Metal using Slang's explicit DXC
*directory* option; the earlier full-DLL-path load failures are retained. That
prototype does not qualify DXIL in AppContainer or production playback. The
existing immutable v15b hardware preview remains unchanged and does not include
these newer ordinary-source features. No whole-game 487-unit build is repeated.


### Confined Windows DXIL compilation

[Run 40676730080](https://github.com/centrexity/GamePKG/actions/runs/40676730080)
passes at `f0d1dc3278265b53ccb68cd374207e3235b5d954`. Both PE32 and PE64 app/game
jobs on AMD64 Windows retain the source and three desktop shader regressions,
then compile signed DXIL through Slang and the pinned Microsoft DXC 1.9.2609 SDK
inside the same AppContainer/owned job. They verify DXBC output and reflection,
cache publication, corrupt DXIL cache rejection on recompilation and the explicit
unsupported-renderer message. Ungranted host includes fail for both SPIR-V and
DXIL, while each same-file deliberate unconfined control succeeds.

The vendor-specific DXC ZIP rule normalizes Windows separators before strict
portable validation. It is confined to that pinned SDK; ordinary tool ZIP and
`.gamepkg` paths still reject backslashes. New fixtures reject traversal,
reserved device names, separator/case/prefix aliases and links. Full official
33-file installation, no-fetch reuse, selected damaged-DLL rejection and repair
pass locally. Both Windows log archives match published sizes/SHA-256 and pass
all CRCs with only the expected logs. This qualifies compilation and integrity,
not DXIL playback or physical graphics/audio/controllers.

The native game-window fixture now passes on actual Windows AMD64 in
[run 40713577705](https://github.com/centrexity/GamePKG/actions/runs/40713577705)
at `d42d7d206ee15e0d580509279622d7a30c2ab4a9`. It builds native-only PE games,
then checks GLSL/SPIR-V/WGSL outputs through test-only software OpenGL: held and
released keys, transparent-black fallback and recovery, resize, normal close,
cached restart and increasing saved play time. All 36 captures pass independent
whole-viewport and letterbox checks, including resized windows. The complete
79-file evidence archive matches its published size, SHA-256 and CRCs.

An earlier capture could include offscreen white edges despite passing center
checks. The fixture now places its own window fully onscreen, records DPI and
capture geometry, and rejects the frozen clipped capture while accepting the
full resized capture. It retains the existing one-level RGB tolerance for the
shader field and requires exact black fallback. The earlier alpha-oracle failure
and clipped captures remain recorded. Python/Pillow are maintainer tools and
all automated audio is muted. The later Windows32/64 Vulkan qualification below extends this software-window
evidence. Physical original-game graphics/audio/controllers remain unqualified.

### Platform admission regression

[Run 40678994989](https://github.com/centrexity/GamePKG/actions/runs/40678994989)
passes at `c3ea1983fc7a3eba3e732a34b7b4c8017186ec30` in both Windows app widths.
The broader portability run had stopped on an obsolete test that expected every
non-Linux native compile to fail: a successful Windows build failed `unwrap_err`,
and macOS expected the older Linux-only shader error text. That failure is retained.
The corrected test preserves unavailable compiler-exchange rejection, reserves
native/shader-unavailable assertions for platforms without those implementations,
and verifies ordinary Windows source-game admission. Unsupported reviewed engine
profiles now reject before tool preparation, staging or build locks/output.
Three exact boundary tests and the existing source/normal-loader/shader/DXIL and
Win32 window/watchdog regressions pass. Both log-only archives match published
sizes/SHA-256 and CRCs. The separate Windows AMD64 game-window qualification above supplies pixel
evidence; these boundary checks alone do not establish graphical behavior.


### Packaged 32-bit native rendering

[Run 40681411252](https://github.com/centrexity/GamePKG/actions/runs/40681411252)
at `cfa8038298cfbaeedb5b7db7fc7fd7ba39888607` passes Linux32, Windows32 and
Android32 jobs. Published artifact sizes, digests, archive CRCs and executable
architectures were verified. Android evidence covers its CLI and emulator tests,
not an Android GUI or native game worker.

The actual packaged Linux ELF32 CLI and GUI also pass local native game-window
checks on a 64-bit Linux host. GLSL, SPIR-V and WGSL outputs pass through software
OpenGL; SPIR-V and WGSL pass through explicitly selected, checksum-pinned private
32-bit Lavapipe Vulkan. Across the five cases, all 25 captured 960-by-720 fields
match every pixel exactly. Tests cover input, texture clearing/restoration,
normal window close with exit zero, cached restart, increasing play history,
and shader/cache/reflection rejection cases. Audio uses null/dummy outputs.
The tests use separate displays, libraries, settings, network namespaces and
private temporary directories. Initial parallel close failures with shared X11
socket paths remain recorded; all five cases pass with private temporary paths.

`8b5ae99` adds these five native GUI cases to Linux32 CI. The first run fails
before rendering because bubblewrap cannot traverse the SDK under private runner
temporary parents. `5382576` places that test SDK under its own traversable `/tmp`
folder without changing compiler isolation or system permissions.
[Run 40725961506](https://github.com/centrexity/GamePKG/actions/runs/40725961506)
then passes all three platform jobs, including the five Linux32 native graphics
cases and packaging. The graphics archive matches its published size, SHA-256
and CRCs with exactly 36 files. Independent inspection verifies all 25 complete
960x720 fields exactly and all five normal-close, cached-restart and increasing
history records. The original failure is retained. This does not qualify a
genuinely 32-bit OS shader toolchain, original-game 32-bit gameplay, physical
GPU/audio/controllers or Android native game delivery.

### Host-independent older-generator reference

The broad all-features [run 40715301601](https://github.com/centrexity/GamePKG/actions/runs/40715301601)
exposes a tiny FPATAN underflow boundary where AMD and Intel physical results
differ beyond C1. The legacy test incorrectly treated the host approximation
as a universal reference. `778b6ee` derives exact-tiny dyadic result bytes, C1,
UE and range wrapping independently with MPFR, retaining all other physical
state checks and the existing numerical-accuracy and monotonicity bounds.

All seven focused tests and all 230 tests across 70 suites pass locally, with
zero ignored tests, strict all-target Clippy and formatting. The complete
616,852,492-byte AMD state corpus is unchanged, SHA-256
`fa7632c0b879c45eb541a066d20f6f47d0be7f5272fcbb1124b90e3af4d2dabd`.
Both opposite-host-C1 and captured-Intel-underflow variants pass; corrupted
implementation C1, UE and numerical bits still fail at the exact boundary cases.
Hosted [AMD EPYC run 40728736496](https://github.com/centrexity/GamePKG/actions/runs/40728736496)
and [Intel Xeon 8370C run 40728749559](https://github.com/centrexity/GamePKG/actions/runs/40728749559)
pass the seven tests and strict Clippy. CPU inventories are checked from each
job, and both one-log archives match published sizes/digests/CRCs. The old
failure remains recorded. This experimental CPU project is separate from the
prepared-Rust game functions and original Takedawn C/C++ player runtime; neither
production numerical code nor game delivery changes. Full-project checks still
need their own terminal results.

### Actual Windows32/64 OpenGL and Vulkan windows

[Run 40728736217](https://github.com/centrexity/GamePKG/actions/runs/40728736217)
at `b2e1b0eaa3575be1093e205eb350eda974c541fc` passes both app widths on AMD64
Windows. Each tests native-only game builds for three OpenGL outputs
(GLSL/SPIR-V/WGSL) and two Vulkan outputs (SPIR-V/WGSL), using private pinned
software graphics. Both archives match published size, SHA-256 and CRCs and
contain exactly 137 expected files. All 120 captures pass independent
whole-viewport/letterbox checks, including resize. Input, exact black texture
fallback/recovery, normal close, cached restart and increasing play history pass.
Actual CLI, GUI, game and graphics-library architectures are checked.

The first Vulkan attempt fails before opening a window. The follow-up native
probe confirms that the CI processes are elevated; the loader normally ignores
an environment-selected driver at that privilege level. The private test loader
honors that explicit selection, without installing system drivers or changing
production loader behavior. Each width confirms its own loader, the exact
pinned Lavapipe DLL hash, Win32 surface support and exactly one CPU Vulkan
device. The test-only loader and probe are not player requirements or releases.

This closes the hosted native source-game window gap for both Windows widths,
including Vulkan. It does not establish genuinely 32-bit OS shader tools,
physical graphics/audio/controllers, original-game Windows gameplay, other
reviewed Windows engine profiles, Android GUI/native workers, full campaigns or
complete multiplayer qualification. Existing preview ZIPs remain immutable;
no full 487-unit cold build, merge or stable release is repeated here.

### Reviewed C compiler startup controls

The pinned Clang frontend still fails its restricted `--version` probe before
any C input is compiled. This remains a failed qualification, and reviewed
Windows C-engine admission stays disabled. The source-game Rust compiler uses
the separately qualified GNU linker path.

The maintainer fixture now pairs that failure with the exact frontend and cleared
environment outside the restricted process, then tests regular-file output with
ordinary and extended executable paths. It also tries a unique temporary
DOS-device name rooted at the already granted toolchain. This last experiment
changes the image's path namespace, grants no additional file access, uses no
drive letter, and removes only its own exact mapping. Creation ownership is
tracked so an unsuccessful creation cannot remove somebody else's device.
It is a test control, not a production compiler fallback or qualified fix.

The hypothesis comes from LLVM's Windows initialization:
[`InitLLVM.cpp`](https://github.com/llvm/llvm-project/blob/main/llvm/lib/Support/InitLLVM.cpp)
obtains its arguments through
[`Process.inc`](https://github.com/llvm/llvm-project/blob/main/llvm/lib/Support/Windows/Process.inc),
which expands the image name before driver options are handled.
[Microsoft documents](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlongpathnamew)
that this lookup needs access to parent directories.
The actual paired controls must determine whether the reviewed binary has that
same cause; source inspection alone does not establish a runtime fix.

Both Windows GNU target widths pass strict all-target Clippy for these controls.
Actual Windows MSVC execution remains required. The final compiler startup,
target-sized COFF/ABI, denied host include and same-file positive-control
assertions remain mandatory and unchanged.

Windows build-tool downloads now allow 900 seconds per transfer. A reviewed
151,102,614-byte archive previously hit the ordinary 120-second deadline after
receiving 90,000,000 bytes, before graphics execution. Package and metadata
requests retain their existing deadline. Tool downloads retain the same
connection deadline, protocol/redirect rules, byte ceiling, pinned SHA-256 and
full payload inventory verification. The eight registry integration tests pass,
and both Windows GNU widths pass strict all-target Clippy. This timeout change
is not evidence that the separate restricted Clang startup issue is resolved.

The [paired Windows32 control](https://github.com/centrexity/GamePKG/actions/runs/40746196201)
at `d83091e` establishes that Clang 23.1.2 starts through the private toolchain
root under the unchanged AppContainer grants. The same frontend and cleared
environment work outside confinement; ordinary and extended image paths both
fail with regular-file output inside it. The private-root version probe succeeds
with private pipes. The old final original-image assertion still fails, so this
run does not qualify C compilation, target ABI or include denial.

The following fixture retains an owned, randomly named root through those
remaining assertions. A separate exact test checks two simultaneous roots,
independent explicit/Drop cleanup, removal from the DOS namespace, and rejection
of relative roots. The helper is test-only until its complete restricted compile
and denial gates pass; no public C-engine admission changes. Both Windows GNU
widths pass strict all-target Clippy. A private Wine API probe cannot open any
of the three custom-device namespace forms (error 66), although exact cleanup
succeeds. That Wine limitation is not evidence against the actual Windows
startup result or proof of Windows process restrictions.

### Shared native C sources and Windows engine links

The native builder now shares pinned C-source staging and reviewed adaptations
between Linux and Windows. Windows stages those same engines, compiles through
the qualified private root, validates every expected target COFF object, builds
the Rust SDK static library with the engine configuration, and links the C
objects and virtual-file wrappers with the managed GNU linker. Directory calls
retain their Windows ABI and remain denied by virtual storage. Compiler execution
has one 180-second deadline after tool installation; tool downloads keep their
separate bounds. The cache identity includes engine sources and compiler pins.

Public Windows C-engine admission remains closed. The actual Windows fixture now
packs and installs both real source packages, builds their native executables,
and checks target identity, integrity and continued admission denial. Those new
link checks must pass before runtime/media qualification and public enablement;
they do not read or upload game data.

The shared staging passes strict Linux and both Windows GNU target checks.
A fresh private Linux run passes 431 library tests (47 separate-fixture tests
remain ignored), native DOOM gameplay/audio/save-and-reload, and native Zelda3
snapshot restoration and exact replay. Selected media are read-only and retain
their hashes; audio is muted/null. A compiled-source fingerprint rejects an
initial stale-library run from the shared target cache. Only the fresh run with
the exact new source plan embedded in the CLI and both gameplay test binaries
qualifies this refactor. No full Takedawn game build is repeated.

The first actual Windows engine-link run passes both engines at i686. At x64,
DOOM links, but Zelda fails because MinGW unwind tables retain unused
emulator-only C routines through section collection. A small local Clang/MinGW
control reproduces that failure; compiling the non-unwinding C code without
unwind tables permits collection. The Windows C builder now uses those flags
and tests both directions with its pinned tools: a dead unresolved call must be
discarded, while a reachable unresolved call must still fail. The unchanged
real signed-engine link checks remain required on both Windows targets. Public
admission remains closed until runtime qualification as well as linking passes.

The internal worker can now initialize the two reviewed C profiles. Public
Build/Play and normal-loader admission remain closed on Windows. A shared
private fixture uses only the exact SHA-256-pinned public Freedoom 0.13.0 phase
1 data: it checks single-player movement and controller forwarding, stereo PCM,
a nonblank full-resolution image, a bounded scoped save, a new worker restoring
that checkpoint, and restart/menu input. It reads diagnostics without changing
game state. No physical audio device or user media is needed.

That exact fixture passes an empty temporary signed-package Build and native
worker run on Linux. The Windows compiler workflow downloads the pinned public
archive, extracts only the fixed expected WAD entry, verifies its separate hash,
and requires the runtime marker as well as the existing compiler/link checks.
It reuses the real just-built signed package and tests continued public-loader
denial before starting the private worker. Actual Windows runtime results remain
pending; Linux success does not qualify Windows CRT or process behavior.

The first Windows runtime attempt at `3d8dfdc` builds and initializes the signed
game at both widths, then reports batch frame zero at the existing 100 ms native
I/O deadline. Public admission stays closed. Bounded first-tick messages now
identify C execution, pixel conversion, image transfer and audio transfer; host
errors identify packet framing and the exact request/reply size and service
time. These are diagnostics, not a timeout increase or gameplay fix. The same
native Linux gameplay/save/reload/restart fixture still passes with the exact
new SDK source embedded in its test binary. Actual Windows traces are required
to locate and fix the stalled phase.

At `dcd8855`, both actual Windows widths complete the C step, pixel conversion,
image transfer and stereo-audio transfer in an early tick before a later timeout.
The next diagnostics retain elapsed microseconds and the last host request,
and distinguish save-state inspection/transfer from end-of-frame diagnostics.
The unchanged Linux native gameplay/save/reload/restart fixture passes; the
Windows cause is still unproven and no deadline is increased.

The Unicode raster correction also passes all 133 rendering tests as a real
ELF32/i386 executable on the local Linux64 host. The test links the real 32-bit
app library and SDL mixer; this is not a physical 32-bit OS qualification.
The 32-bit Linux/Windows/Android workflow now includes the complete game suite
(the SDL-only comparison is Linux-specific). Android's runner previously built
the render-resource suite without executing it; it now runs every named suite.
Its filesystem-dependent signing fixture is embedded byte-for-byte so the
device needs no checkout. Three black-box controls compare compiled suites
with actual runner invocations, require failure propagation and reject a
missing binary. The four resource tests and strict lint pass locally; actual
new Windows/Android execution remains required.

The actual `04030c0` traces correct the earlier frame interpretation: the fixture
reset its local frame index for every input batch. The first complete frame
takes about 1.6 ms; a later tick fails before making any host call. Error labels
now include tick state, input and previous engine diagnostics. Native DOOM's
Windows save paths also used a backslash while its virtual store accepts fixed
forward-slash names. Native source staging now preserves that canonical spelling
on every OS and the C compiler asserts it. The store accepts no new aliases.

Native stdio now formats into a bounded buffer and forwards diagnostic bytes
through Rust stderr, without passing a CRT-owned FILE object across libraries
or contaminating the protocol stream. The worker selects DOOM's `-nogui` error
mode so a fatal loading error cannot block on a worker dialog. Cache identities
include both native stdio adapters. Linux native gameplay/audio/save/reload and
a RAM-only missing-PLAYPAL control pass; the latter must report the original
engine error without a timeout. Windows qualification remains required.

Hosted `04030c0` canonical delivery, Linux32 and Android32 gates pass completely.
The canonical gate includes the exact full-size decomposed-name typing fixture
in all three runtimes and complete browser/legacy checks. Linux32 runs all 133
game tests; Android actually executes all eight named suites, including 132
game tests (the SDL-specific comparison is Linux-only) and four resource tests.
The added Windows32 suite exposes five original procedural-scene reference
failures, including maximum frame-counter cases. These remain blocking; no
goldens or tolerances are relaxed.


### Current native worker and normal-launcher follow-up

The Windows save-path/stdio revision `286b72c` passes the complete reviewed
C-engine qualification at both executable widths in run
[40781018454](https://github.com/centrexity/GamePKG/actions/runs/40781018454).
Both signed engine links and the public-Freedoom private-worker gameplay,
controller, stereo PCM, full image, checkpoint reload, restart and missing-palette
fatal-error controls pass. Published archive digests, sizes and ZIP CRCs are
independently verified. This does not qualify original user media or physical
Windows graphics/audio/controllers.

The portable-effect revision `03e4378` routes binary32 effects through the
existing deterministic reduction and preserves the original binary64 fire-brick
calculations. The final local 64-bit and actual ELF32 suites each pass 134 tests,
including 3,607 independent 512-bit MPFR reference cases and unchanged original
scene pixels/commands. Exact scalar/glyph property caching also removes repeated
Unicode lookups. Complete signed native/core/component maximum-name typing,
music/effects, checkpoint/restart, corrupt-save preservation and six dense
original-artwork scenes pass with unchanged deadlines and memory limits. Hosted
current Windows32/Android32 qualification remains pending.

The next draft enables only the reviewed DOOM C profile in ordinary Windows
Build/Play; Zelda and other unqualified C profiles remain gated. The normal Game
loader fixture checks explicit and default native selection, controller input,
stereo PCM, full image, actual checkpoint-slot reload, increasing play history,
and rejection of missing ROMs and damaged cached artifacts. Configuration-only
bundles do not satisfy the checkpoint assertion. This complete signed Linux
fixture passes with public Freedoom and silent output; 337 default-feature
library tests pass with 46 separately ignored fixtures. Strict lint passes on
Linux with all features/targets and for both Windows GNU executable widths.
Actual Windows normal-loader and game-window qualification remains required
before treating the draft as complete. No stable release or preview replacement
is made by these changes.


The normal-launcher revision `a9730aa` now passes its actual Windows x64
compiler/link/worker/normal-Game-loader job (`123162507564`, run `40786978771`).
The 2,004-byte evidence archive `12860172001` matches SHA-256
`0add180c07e5f39e24f3d232fb4d4f87a1edb48d0914983ec3fa2f2e2d51eda2`;
both expected log entries and ZIP CRCs verify. It includes the explicit
qualified-profile admission controls and the complete normal-loader marker,
including populated-checkpoint reload and increasing play history. The i686 job also passes: its 2,010-byte archive `12860292535` matches
SHA-256 `858c0f65e074213b58a7d2337c4cd1ad0a57c3d9b3262acbdd59a3520448da55`,
with both logs, ZIP CRCs, admission controls and normal-loader markers verified.
Both are actual Windows tests; the i686 executable runs on a 64-bit OS.

The Windows graphics workflow now adds `tests/windows_doom_gui.py` after the
existing exact shader/viewport checks. It builds the signed DOOM recipe through
the normal default CLI Build, rejects implicit Wasm output, and opens the normal
GUI with a private remembered path to the separately pinned public Freedoom WAD.
Each OpenGL/Vulkan first launch and restart checks visible colored gameplay,
physical keyboard movement/save-name input, a populated original checkpoint,
original F11 gamma persistence without changing window size, resize, normal
close and increasing history. Only the process/window created by the fixture
receives input or capture. Audio stays muted, selected WAD bytes stay unchanged,
and no test hook is added to the app. The Win32 save-file reader shares delete
access so observing an atomic save cannot block its replacement.

Five local verifier controls reject configuration-only bundles, malformed or
oversized entries, missing/duplicate/invalid gamma, blank/gray output, thumbnail
output and nonblack letterbox pixels. These controls and Python/YAML syntax
checks pass locally. The new actual Windows GUI gate is pending. Its captures
qualify presentation/input integration, not independent pixel-perfect game
fidelity or physical GPU/audio/controller behavior; existing exact shader and
original-engine reference comparisons remain separate requirements. Python and
Pillow are maintainer tools only.


The complete `03e4378` portability run `40785617177` now reports success for
Linux32, Windows32 and Android32. The Windows runtime-stage log explicitly
executes the independent MPFR test and original procedural-scene suite; the
previous five Windows32 differences are fixed without changing expected scenes
or tolerances. The Windows job also completes the standalone compiler/worker,
matching CLI/GUI build, relocated binary checks and package. Its terminal job
and log are independently retained; this is not a physical 32-bit OS claim.
Linux32 and Android32 terminal results still require their separate detailed
log audit before using them as expanded qualification evidence.


The initial Windows GUI run `40789067293` reports success at both widths.
Its two 186-file evidence archives match published size/SHA-256 and ZIP CRCs;
all 160 PNGs decode. Independent checks confirm 120 complete shader fields and
all eight DOOM launch/history/gamma routes. Visual inspection confirms original
gameplay/HUD in game-only windows. This is partial GUI qualification: the audit
also caught that restored window preferences made three of four resize calls
per width no-ops. Each width actually resized only its first OpenGL launch.
The strict failed audit is retained and the older results are explicitly labeled
partial; they do not establish all-route resize coverage.

The follow-up chooses alternate outer dimensions based on the current client
width, requires a changed captured client size before accepting the resized
frame, and records both sizes with the history proof. Product window preferences,
rendering, game data and deadlines are unchanged. The stronger actual Windows
OpenGL/Vulkan first/restart checks remain required. Five local adversarial
verifier controls and syntax checks still pass.


The first stronger-resize run `40791395885` fails both widths at the first
Vulkan launch restored from the taller OpenGL window. Both OpenGL launches
finish, including actual geometry changes. Captures show a white stripe in
Vulkan's bottom rows. The same run's shader-window metadata records a
1024x768 desktop; the fixture requested a 1000x800 outer window. With the
39-pixel window frame its 761-pixel client exceeds the visible desktop below
its caption. Both renderers stop capturing game pixels at row 737; OpenGL's
remaining captured rows are black and Vulkan's are white. This is evidence of
an invalid offscreen measurement; it does not yet prove a game-rendering bug.
Both immutable failed archives and logs are retained.

The follow-up queries the owned window's monitor/work area and frame dimensions,
places it wholly inside that area, and derives alternating wide/tall sizes from
the available client dimensions. Every capture asserts full client visibility
and records client origin, outer/monitor/work rectangles, screen dimensions and
DPI. Actual size changes, black letterbox fields and game-image checks remain
mandatory. Six local controls cover malformed save/gamma/output and alternating
visible geometry across three work-area/frame configurations. The corrected
actual Windows runs remain required; neither the offscreen captures nor local
math controls qualify those runs.


The complete hosted canonical run `40791404759` at `6995119` succeeds with
all steps complete. Its retained terminal log verifies 134 game tests, complete
original artwork/audio, full-size decomposed-name editing across all three
runtimes, browser playback/exports and 24 baseline/recorded core/component dense
scenes with maximum saves. Both signed legacy suites run all 72 tests. Original
source data, expected pixels, tolerances, memory ceilings and deadlines are
unchanged. This supersedes the earlier pending/cancelled canonical statuses.

Broader Rust CI at `a1af190` stops at a new Clippy 1.99 lint in the private
missing-palette fixture: constant-size mutable chunks should use fixed arrays.
The helper now uses `as_chunks_mut::<16>()`, retaining every exact WAD directory
entry and the assertion that one palette entry changes. Rust 1.89 minimum-version
compilation and strict all-feature/all-target lint pass locally, and the complete
public-Freedoom native worker/normal-loader plus fatal-error control passes again
with unchanged WAD bytes and silent output. No warning is suppressed and no
runtime, scene or failure assertion is relaxed. Corrected hosted broad checks
remain required.

## Independent player-two normal-input probe

The maintainer bridge in `tools/takedawn_input_probe.py` selects either player,
uses that player's original phase and pause owner, and prefixes all pad actions
for the selected controller. It supports physical Xbox and PS2 pedal/boost
layouts. The runtime applies the original controller assignment and layout;
the bridge writes only ordinary virtual-pad commands and capture requests.
It does not write game memory, activate AI or alter race outcomes. Players need
none of this Python tooling.

A bounded local Linux session of the exact signed v15 native executable
(`621988980facdace69fc4dbf1e2b6de4043b442dd8d910854f09df38002b30df`)
passes player-two acceleration, left/right input acceptance, pause owned by
player two, normal Resume and normal driver shutdown. Original controller reads
show physical PS2 acceleration reaching the game's RT input. Three complete
640×480 game captures remain nonblank; the player-two viewport changes on both
steering steps. Visual inspection confirms split-screen gameplay. Strict path
and actual Vulkan gates pass, audio uses the dummy output, the ISO is read in
place and reference profile/binary hashes remain unchanged. This qualifies the
probe's routing and active game presentation, not exact steering equivalence,
physical controllers or a completed multiplayer race.

The first launch failed before the game started because its copied launcher
lacked execute permission. A later checker incorrectly included player-two menu
A presses from before the race layout was active. Both failed reports are
retained locally; the accepted check scopes actual controller reads to the race
start. No game code, layout, deadline or assertion about driving inputs changed.
Three dependency-free adversarial tests cover player selection, physical layout
commands, input-syntax rejection and wrong-player pause/finished-player guards.
They run in both broad CI configurations without Pillow; capture operations alone
load Pillow when needed. Player-two natural finish, mixed result prompts and
normal Continue after shared results remain open.

## v16 shared-results replay input correction

The normal signed v15 executable exposed a real Team Crush results bug. The
original shared results menu runs a looping replay behind its score table. Both
cars return to phase 2/countdown state, while neither per-player results byte is
set. The shared word at `0x752D30` is `0x01010000`: the original replay byte and
no-pause byte are both set. The previous multiplayer gate treated those replay
cars as driving. A fresh physical A became RT and could not select Continue;
the prompt displayed the mapped shoulder button instead. A shoulder press did
select Continue and reached the original autosave screen. Road Rage gameplay,
its normal pause/Quit confirmation and return to mode selection also passed in
that baseline session; no completed Road Rage event is claimed.

The reviewed v16 policy keeps the original menu layout when both shared flags
are set. No-pause alone still cannot disable the other player's live controls
after one finishes. Existing player assignments, pause/held-button handover,
Nintendo face layout and all 15 glyph families remain intact. The new recorded
state regression fails before the fix. The complete staged source then passes
261 checks on both 64-bit and actual 32-bit Linux with address, undefined-behavior
and leak sanitizers enabled. These are source checks, not Windows hardware proof.

The 0.1.5 candidate's source kit SHA-256 is
`d11056c7b8e1306023b0229dd32f6afb60194fd786b73e3abf187210eeb0de5f`.
Its inventory differs from v15 in exactly one file, the host C input policy.
Prepared Rust, ABI support and every other kit file are byte-identical. The
reviewed prepared-unit cache namespace is therefore retained, while kit admission
and final artifacts use the distinct new identity. Every expected source/compiler
argument/cache key matches one of 487 checksum-verified copied v15 objects.
Normal signed key generation, packing, installation, Build and integrity checks
pass; Build takes 24.56 seconds. All 24,035 exports remain present, and both the
original and copied objects preserve their hashes and timestamps. This is a warm
build, not another cold qualification.

The first two build attempts stopped before any game-unit compilation: a private
settings override used the wrong variable, then the restored glslang prefix
lacked its standalone tool required by the package's CMake metadata. The accepted
prefix is private and uses GPG-verified Ubuntu updates metadata plus package
hashes for glslang 15.1.0, SPIR-V Tools 2025.1 and matching tools. No system package
is installed. Another driver audit wrongly expected successful compiler stderr
to be forwarded to the CLI; its failure is retained separately from the actual
successful Build/integrity and verified artifact/cache identities.

The resulting native executable SHA-256 is
`329c48d0a3eba805d389043732eebc2a4a030881b180520dd3404c6db741367f`.
A fresh bounded normal-input Team Crush session passes the original results flow.
A is pressed while both replay cars are in phase 2 and both shared flags are set;
actual controller reads preserve A and return RT zero. The original A Select and
B Replay prompts are restored. Complete 640×480 captures show $3,357 plus $3,380
vehicle damage and a $20,000 cash bonus, totaling $26,737. Normal Continue reaches
**Autosave successful**, and the private profile changes. Final capture is that
autosave confirmation awaiting another Continue, not a captured return to mode
selection. The reference profile and executable hashes remain unchanged; strict
native-path and actual-Vulkan checks pass. All automated audio uses dummy output.
The first fixture pressed A during the camera fly-by; its corrected version
waits for the original instruction-page flag, with no runtime or timeout change.

Targeted host tests pass (24, with nine explicitly separate checks ignored), the
explicit current source-kit archive test passes, strict all-feature/all-target
lint passes, and both CLI and GUI build with embedded audio companions. Relocated
entry points validate the same decoder without a display or decoder files. The
catalog matches all six dependency-free recipes and their original artwork
notices.

This closes the observed Linux Team Crush results-input defect. The subsequent
Windows runtime/Wine and fresh-process record checks below extend that evidence.
Remaining native control routes, the current signed Windows app Build/Play path,
player-two natural Race finish/Continue, mixed prompts and broader
campaign/hardware fidelity remain separate work. Recipe 0.1.5 requires updating
the package and rebuilding; existing immutable v15 previews contain recipe 0.1.4.
The full Windows cold-build workflow is explicitly dispatched release
qualification; maintaining its kit pin does not automatically repeat the complete
487-unit build. Its retained v15 results do not qualify v16.


### Current v16 Windows runtime and results replay under Wine

The v16 Windows runtime now rebuilds successfully using the exact pinned
standalone Windows SDK under Wine. The recovered private SDK inventory matches
`c26f09324eda2a9f8091bf7330d5363bd22f4cee9a98fb93fe58bbe3eb02ec67`.
All 494 prepared Rust and ABI files are identical to the qualified retained
archive. Each of the 487 current source/compiler-argument/cache keys and object
receipts matches; the archive hash remains
`59b5eb0baff73b051cf7f8b429b718df64a881d6304d9dc92c1b2c980861562c`.
The actual Windows helper compiles with standalone Rust 1.89 under Wine, and the
C/runtime rebuild takes 64.46 seconds. No complete Rust-unit rebuild is repeated.
All 24,035 exports remain in the resulting native Windows executable, SHA-256
`5201c4c976ffebbed58117fc29569f054a1ea4bf9da487098ca4c41e97c465d8`.
This is runtime integration with a verified reused archive, not a new signed
Windows app Build/repair or cold-build qualification.

That exact executable passes the original two-player Team Crush results route
under Wine in a private virtual display. The owned ISO is admitted read-only in
place and automated audio uses dummy output. Normal physical PS2 virtual-pad
inputs select the mode, junction and cars and accelerate both players; no game
state, AI or result is injected. During the shared results replay, both car
phases are 2 and the shared word is `0x01010000`. A fresh A at flip 5,611 remains
A with RT zero. The complete 640×480 result capture shows the original A Select
and B Replay prompts, $3,359 plus $3,377 damage and the correct $6,736 total.
Continue reaches **Autosave successful**, with a changed private profile. The
final capture still awaits Continue on that confirmation; mode selection and
fresh-process persistence are not claimed. All five captures pass independent
full PNG decoding and hash checks. Strict executed-path and actual-Vulkan gates
pass, and the reference profile and executable remain unchanged.

This extends the observed results-input fix to the Windows binary under Wine.
Actual Windows graphics/audio/controllers, the normal current Windows app
Build/Play path, fresh-process profile behavior, other native control routes,
player-two natural Race completion, mixed viewport prompts and broader campaign
fidelity remain separate qualification. Existing preview archives stay unchanged.


The current Linux CLI additionally passes ordinary default-native Run with the
signed v16 package: an invalid ISO is rejected, an explicitly selected in-place
ISO renders 120 frames, and a fresh invocation renders 240 frames using the
remembered selection. Persisted play time increases from 2,030 to 6,002 ms.
The actual compiled cache artifact matches the qualified v16 executable and its
receipt; app, signed package, settings and retained native-copy hashes remain
unchanged. Audio is muted and the watchdog stays zero/off. The first audit
looked for a ROM-selections directory instead of `rom-selections.json`; that
audit failure is retained. An independent audit reads the actual saved file,
checks both successful Run outputs and increasing history, without repeating
Build or Play. This establishes the current CLI launch path, not GUI interaction
or loading the earned Team Crush result into an original profile screen.


### Current v16 earned-profile reload on Linux and Wine

Fresh original game processes now load the earned profiles from the accepted
v16 Team Crush sessions and display their saved records. These checks copy each
closed session's actual private save files into a new private game directory;
they never edit profile contents, game state, AI, cash values or outcomes.
The admitted native binaries are unchanged. The owned ISO is read-only in place,
audio uses dummy output, and normal virtual-pad menu presses load Profile 1.

In the Linux process, **Driver Details > Records > Team Crush > USA** displays
**Trailer Trash**, players P1/P2 and **$26,737**, exactly matching the preceding
original results capture. Driver Details also shows 2/173 completed events,
4,060 burnout points and six takedowns. The junction selector itself does not
show a high score; the independent records page supplies the persistence proof.
Seven complete 640×480 captures decode and match their hashes. One external
observation helper could not see the owned process from the restricted process
namespace after it had submitted a normal Back press; inspection continued in
the same original session with the appropriate process visibility. No game or
harness source was changed while running and no session was restarted.

A separate fresh Windows executable process under Wine loads its own earned
profile and follows the same original records route. **Trailer Trash** shows
P1/P2 and **$6,736**, exactly matching that Windows/Wine results session. Both
complete captures decode and match their hashes. This is a separately earned
result, not a copy of the Linux score or an exact simulation-equivalence claim.

Both owned processes exit zero, pass strict executed-path and actual-Vulkan
gates, and leave every UDATA profile file and reference file byte-identical.
The native executable hashes also remain unchanged. These checks establish
original earned-record persistence across fresh processes for the current v16
Linux and Windows-under-Wine runtimes. They do not qualify the current app's GUI
launch path, the normal signed Windows app Build/Play integration, physical
Windows graphics/audio/controllers, natural player-two Race completion or the
remaining campaign/multiplayer routes. Existing previews remain immutable.


### Current Linux native app game-window integration

The current bundled `gamepkg-gui --play com.gamepkg.takedawn` entry also passes
with the signed v16 package. Explicit private `--library` and `--config` flags
override deliberately invalid inherited locations; the wrong library is never
created and the inherited settings file stays unchanged. The remembered ISO
starts the native executable in a separate SDL window titled Takedawn. No
GamePKG library window or additional app controls open. Its window PID is 2 in
the game's PID namespace, distinct from the app process; this is not a host PID.

Real X11 keyboard events, through the unchanged SDL keyboard map, load the
copied earned profile and navigate **Driver Details > Records > Team Crush >
USA**. The actual 960×720 game-window capture displays **Trailer Trash** and
**$26,737**. It and the main-menu capture pass full PNG decoding and hash checks.
A normal `WM_DELETE_WINDOW` closes the game and app with exit zero. Persisted
play time increases from 6,002 to 57,024 ms. All saved profile/reference files,
app/game/package bytes and explicit/inherited settings remain unchanged. Actual
Vulkan and strict executed-path checks pass.

The private display never appears on the user's desktop. A separate private
PulseAudio server has exactly `module-null-sink` and
`module-native-protocol-unix`, a single `gamepkg_silent` sink and no hardware
modules; the actual app settings additionally mute audio and leave the watchdog
off. Its runtime socket is the only audio socket exposed to the game. The
private audio server exits zero after the successful check.

The first window test reached the correct saved record, but the Xlib close
helper read its parent display credentials instead of the private display's
credentials. Its terminal failure and forced cleanup are retained; the owned
session lock was confirmed released. Only that helper was corrected before a
fresh test with the same frozen app/game source and unchanged normal close
protocol. No production change, assertion relaxation or live source edit was
used. The second complete check passes. This qualifies the current Linux direct
GUI game entry and normal window lifecycle, not clicking Play through the
library page, current Windows app integration or physical hardware.

### Allocated storage and the actual library Play route

The current Linux library's visible Play control is now exercised, rather than
only invoking the direct `--play` entry. It launches the separate native process
with the selected library, remembered in-place ISO and unchanged v16 native
executable. Real keyboard input loads the earned profile and displays the
original $26,737 Trailer Trash record. Normal window-manager close removes and
reaps the player, keeps the library running, and refreshes its Played/readiness
labels without pressing Refresh. The library and private null-only audio server
then exit zero. All profile/reference files, app/game/package/settings bytes and
native-path/Vulkan checks pass. Eighteen complete original captures decode and
match their recorded hashes.

This exposes a storage-reporting defect: the original sparse Xbox disk images
have roughly 5.1 GiB of combined file sizes while the game's actual inventoried
files occupy about 174 MiB. Storage now keeps both values. Linux/Unix uses
OS-reported 512-byte file blocks; Windows asks for compression/allocation through
an open regular-file handle that does not follow reparse points. Unsupported
allocation queries stay unavailable: the UI says **in files**, not **on disk**.
Logical category counts, `total()`, `used_bytes()` and existing JSON fields retain
their previous meaning. Desktop, CLI and browser displays use allocation when
available, and browser v1 snapshots without the new fields remain readable with
explicit file-size labels. Filesystem metadata and exclusive extent ownership
are outside this file-allocation measurement; shared storage can be attributed
to several game entries.

All five targeted storage tests pass, including a 6 GiB-plus-123-byte sparse
native disk with an actual write beyond 4 GiB, creator scoping, external-link
exclusion and unavailable-allocation wording. An actual ELF32 backend probe
reports 6,442,451,067 logical bytes and 4,096 allocated bytes, agreeing with GNU
`du`. Its initial restricted execution was stopped by the tool's 32-bit syscall
filter; the authorized host execution passes. Strict all-feature/all-target
Clippy and bundled CLI/GUI release builds pass. Development checks use no debug
symbols or incremental copies to avoid recreating the previous oversized caches.

The actual Linux game page displays **174.0 MiB on disk**. Independent filesystem
counts give 182,427,648 bytes for the per-game inventory and 182,431,744 bytes
for the library, versus 5,425,521,550 logical library bytes. The table/footer and
page agree with those counts. Normal Play, the original saved record, close,
reaping and automatic history/readiness refresh pass in the same final session.
The authenticated headless browser test also passes new allocation fields,
visible labels, legacy cached metadata, icons, achievement text and cross-origin
rejection.

Both PE32 and PE64 versions of the exact backend compile and execute under Wine.
Wine returns **Invalid function** for handle compression information, while its
independent compressed-size API returns the logical length for this sparse
fixture. This proves the honest unavailable fallback and executable-width ABI;
it does not qualify native Windows allocation. The Windows graphics workflow
now runs a strict sparse-allocation storage gate at both app widths on real
Windows, requiring agreement with the independent compressed-size API. The
patched-engine run [41325179420](https://github.com/centrexity/GamePKG/actions/runs/41325179420)
at `f2d8e8e` passes all four applicable storage tests at both MSVC executable
widths, including the sparse write beyond 4 GiB. The full graphics workflow
also completes successfully at both widths. This qualifies actual Windows file
allocation; the Wine result remains limited to its unavailable-query fallback.

Private fixture failures are retained separately: a too-long null-audio socket
name, text recognition over animated screens, timing assumptions in normal menu
navigation, and an interrupted private display. Original captures remain intact;
the final run uses the observed short-tap route and an extra recognition pass.
No production renderer, game state, deadline, memory ceiling, expected score or
storage requirement was weakened to obtain a pass. The first current-head
full-suite ancestor run also has an independent Arkanoo maximum missing-mark
name-edit timeout in core/component Wasm, while its native counterpart passes;
this remains open and is not an allocation-test success or a full-suite pass.


### Adjacent raw-image snapshot reuse

At `f2d8e8e`, the push portability run 41325172841 passes Linux32 and Windows32
but Android32 interrupts the component large-image quota fixture on its existing
100 ms tick deadline. Core/component and basic portability suites otherwise
pass before that failure; this is not an Android-wide qualification.

Raw core/component image submission now compares the complete pixel bytes and
exact dimensions with the immediately preceding immutable image. An identical
image shares that snapshot rather than allocating and copying it again. A
changed byte or shape still creates a private copy. No guest memory is retained,
no cross-frame cache is added, and every draw still charges its full transfer
size and command count. Clear keeps its existing transfer charges.

Fifteen local drawing tests pass (one cold-image measurement remains explicitly
ignored), along with all 11 component and five resource tests and strict
all-feature/all-target Clippy. Both runtime profiles verify actual snapshot
identity at the full 8/32 MiB quotas, retained pixels across guest mutation and
subsequent ticks, and distinct nearest/linear command positions. A maximum-size
image differing only in its last byte cannot reuse the earlier snapshot. The
100 ms deadline, memory ceilings, accepted counts, and pixel assertions remain
unchanged. New hosted Android evidence is still required before calling the
timing failure resolved. Native game source and immutable previews are unchanged.


### Matching Windows app preview artifacts

Successful native Windows graphics jobs now package the matching CLI/GUI,
dependency notices and sources using the normal release packager. Artifacts are
named with the source head and executable architecture and include a separate
PREVIEW.json recording both source head and actual checkout commit. Packaging
runs only after shader, Vulkan, DOOM first/restart and sparse-storage checks pass.
It checks executable architecture and the standalone CRT requirements, then
preserves the normal archive SHA-256 sidecar. Test-only graphics drivers are not
added by the release packager. These are unreleased app test artifacts with
seven-day retention, not replacements for published releases or immutable
previews. They allow the current app to be exercised under Wine without another
full 487-unit game build. Current artifact creation and runtime integration
still require their own successful results.


### v16 original Double Impact results and score continuation

The immutable native v16 executable (SHA-256
`329c48d0a3eba805d389043732eebc2a4a030881b180520dd3404c6db741367f`)
now passes an additional original Double Impact route. Normal PS2 virtual-pad
inputs select the mode and Trailer Trash junction, start both players and
accelerate both cars. Each player naturally reaches the event-finished phase
before the shared results replay begins. No debug AI or game-state writes are
used. The actual result is P2 wins: P1 $3,357, P2 $23,380, including the original
$20,000 cash bonus.

During the active background replay, both original car phases are 2. Normal A
is read as Xbox menu A with RT zero, opens the separate SCORE menu, and a second
normal A selects its Continue action. The original Multiplayer mode selector
then displays the retained P1/P2 total wins 0–1. Six actual 640×480 captures
fully decode and match their recorded SHA-256 hashes. Strict native-path and
actual Vulkan checks pass; executable and reference-save hashes are unchanged.
The supported ISO stays read-only in place and audio uses the dummy device.

Two earlier observations remain failures of the adapted probe, rather than
being relabeled as passes: one awaited the active replay phase before preparing
its capture; the next expected the front-end module immediately after the first
Continue and captured the intervening SCORE menu instead. The qualified probe
prepares capture at replay initialization after observing both natural finishes,
retains the active-phase confirmation check, verifies SCORE/RETRY text, and
follows the original second Continue action. Its 45/30/20-second observation
limits and 360-second outer bound remain unchanged.

This qualifies the original results-to-score-to-mode-selection route. It does
not establish player-two natural Race completion, all campaign events, complete
physics/audio equivalence, physical Windows behavior, or normal window-manager
close: the headless harness ends this owned process after the final capture.


### v16 natural three-lap Race completion for both players

The unchanged native v16 executable (SHA-256
`329c48d0a3eba805d389043732eebc2a4a030881b180520dd3404c6db741367f`)
completes an original three-lap two-player Race on Linux with zero CPU rivals.
The original menu exposes region, track and rivals; no lap-count option is shown.
The visible HUD confirms three laps and positions out of two. Both players reach
their final lap and naturally enter the finished phase: P1 at flip 22,603 and P2
at flip 24,599. The original results show P1 **1st, 10:20.03**, best lap **2:38.53**;
P2 **2nd, 11:26.56**, best lap **2:25.15**. This is a distinct natural result,
not the earlier 30-minute timeout/Draw observation.

Inputs are ordinary Xbox pad reports through the existing static/live input
interfaces: both right triggers stay held while live commands steer, brake and
reverse. Three logged recovery sequences free cars from roadside obstacles.
The private launcher forwards the existing static-input variable; the game
executable, prepared functions and original physics are unchanged. Debug AI,
car-position edits, lap/result writes and outcome injection are absent. Normal
A traverses Race Results, Score with wins 1–0, **Autosave successful**, then the
Multiplayer mode selector retaining that win count.

All 69 recorded 640×480 captures fully decode and match their hashes. Strict
native-path and actual Vulkan checks pass on an NVIDIA GeForce RTX 3090
(Vulkan 1.4.312), with dummy audio and the supported ISO read-only in place.
The original reference-save hashes and executable hash remain unchanged; only
the private copied Profile 1 changes after saving. The owned engine and driver
are confirmed absent after the bounded headless harness ends the session.

This qualifies the selected three-lap zero-rival route and its normal
results/score/autosave/menu flow. Fresh-process reload of the new Race records,
default/nonzero rival configurations, other tracks/campaign events, complete
physics/audio equivalence and physical Windows controls remain unverified.
The harness stop does not qualify normal window-manager close; that separate
app/window integration retains its previously recorded scope.


### v16 earned Race records survive a fresh process

A fresh process of the same unchanged native v16 executable loads a private
copy of the profile earned by the natural three-lap two-player Race above.
Normal menu input opens Driver Details → Records → Racing → USA, then the
Downtown Southbound records. The original Top Lap Times screen displays
**P2 2:25.15** and **P1 2:38.53**, matching both best laps from the completed
race. This verifies actual saved-record consumption, beyond a successful
profile load or merely finding changed save bytes.

All five UDATA/TDATA profile files retain their exact hashes after reload, and
the preserved earned snapshot and executable remain unchanged. Seventeen
640×480 captures fully decode and match recorded SHA-256 hashes. Existing
strict native-path and actual-Vulkan checks pass on the NVIDIA GeForce RTX3090
(Vulkan 1.4.312, headless); audio stays dummy and the owned ISO is read in place.
The runtime creates its ordinary private cache partitions, logs and backups;
those generated files are separate from the unchanged profile comparison.

The original observation session exits143 while the owned game remains alive.
Inspection resumes through normal pad input in that same process, with its
owned path and PID/start identity checked; the game is not relaunched. The
final owned scene-stop marker ends the game, and the owned driver and engine
are confirmed absent. Long directional holds repeat in the original menu;
short normal taps select the intended records page. No state, car, lap or
result is injected. This is headless harness termination, not another normal
window-manager-close qualification.

This closes fresh-process reload for the selected earned Race lap records.
Default/nonzero rivals, further tracks and campaign events, exact physics/audio
fidelity, original online behavior and physical Windows qualification remain
open. The immutable Windows source preview and native game package are unchanged.


### v16 original Race with the default four CPU rivals

The same unchanged native v16 executable completes another original three-lap
Downtown Southbound split-screen Race with the default **four CPU rivals**.
The original HUD displays positions out of six and lap 3/3. P2 naturally enters
the finished phase at flip 25,078; P1 follows at flip 31,957. Before its finish,
P1 is visibly driving the final lap at 119 mph at flip 31,622. The original Race
Results show **P2 fifth, 11:57.71**, best lap **3:22.23**, and **P1 sixth,
15:47.01**, best lap **2:31.18**. Both results precede the original 30-minute
race timeout; this is not another timeout/Draw observation.

Both right triggers are held through the existing static input interface.
Logged ordinary steering, brake/reverse maneuvers and boost free the cars from
mutual collisions, curb contact and traffic pileups. Original crash replays and
recovery remain active. In particular, after a prolonged final-lap blockage,
P1's normal boost and subsequent original crash/recovery lead back to visible
forward driving and its finish. Both player cars receive normal pad input and opponent AI is unchanged. No
debug autopilot, position/lap/result injection or physics change is used. This control-driven run is not a pacing/physics parity
measurement against original hardware.

Normal A traverses Race Results → Score (P1 wins 0/P2 wins 1) → **Autosave
successful** → Multiplayer mode selection retaining 0–1. All 70 recorded 640×480
captures fully decode and match their SHA-256 hashes. Strict executed-path and
actual-Vulkan checks pass on the RTX 3090 (Vulkan 1.4.312, headless), with dummy
audio and the owned ISO read-only in place. The original five-file reference
profile and executable remain unchanged; only the private copied Profile 1
changes after saving, and a separate earned-profile snapshot is preserved.
Fresh reload of this specific later snapshot is not claimed; the earlier
zero-rival earned-record reload retains its separate proof.

The owned observer, driver and engine timeout are confirmed absent after the
normal owned headless scene-stop command; the driver exits 0. No native game
rebuild, source-kit/package update, immutable Windows preview replacement,
normal window-close qualification or stable release is part of this check.
Other tracks/rival counts, remaining modes and campaign events, exact
physics/audio equivalence, original online functionality and physical Windows
qualification remain open.


### v16 default Party Crash Solo turns and final standings

The unchanged native v16 executable completes the original Party Crash defaults:
Solo, two players sharing the same controller, one round, locked car type and
random junctions. The selected junction is Trailer Trash. Normal Xbox A starts
each turn, followed immediately by a held right trigger for 30,000 ms. Each turn
reaches its original finished phase and shows its own **Round 1 of 1 Results**
plate: **$84,633 vehicle damage**, $0 cash bonuses, no multiplier, and **$84,633
Crash $**. The original Player 2 Get Ready screen verifies the turn handoff.
No score, car, traffic, player-state or outcome is injected.

Fresh normal A presses traverse the second result plate, **Final Standings**,
the original Continue/Retry choice and the main Select Option menu. Final
Standings lists Player 1 first and Player 2 second, both at $84,633; no Draw label
or inferred tie-break rule is invented. Result text is used only to recognize
when a table is visible. Both numerical score values and player labels are
separately visually checked because raw OCR sometimes misreads individual digits.

All 30 actual 640×480 captures fully decode and match their hashes, including
retained loading/animation observations. Strict executed-path and actual-Vulkan
checks pass on the RTX 3090, Vulkan 1.4.312, headless, with dummy audio and the
owned ISO read-only in place. The executable and all five reference/private
UDATA/TDATA profile files remain unchanged. No autosave prompt or new profile
write is observed, and saved Party Crash score persistence is not claimed.
The owned observer, driver, engine timeout and native engine are confirmed
absent after the normal owned scene stop; the driver exits 0. This is not another
window-manager-close, app-history or physical Windows qualification.

The first bounded observation remains incomplete: its operator-delayed first
turn produced a $0 result and reached the Player 2 handoff, but a later snapshot
landed in a replay loading fade and final standings were not captured before
its interaction window ended. The qualified probe starts turns promptly with
normal inputs and waits for visible vehicle-damage/cash-bonus/result text,
retaining all intermediate captures. The original 600-second outer and 450-second
interaction bounds are unchanged; no runtime guard or game source is changed.

This qualifies the selected default Solo/two-player/one-round route. Other
party types, player/round counts, junctions, cash/multiplier/Crashbreaker branches,
exact original-hardware physics/audio, campaign and physical platform scope
remain open. No native game rebuild, package/source-kit update, immutable
Windows preview replacement, stable merge or release is part of this check.


### v16 two-round Party Crash cumulative standings

A further normal-input check selects Solo, two players and **two rounds** in the
original Party Crash options. Locked car type and random junctions remain Yes;
the game selects Trailer Trash in both rounds. The four Get Ready screens show
Player 1, Player 2, Player 1 and Player 2 in that order. Original A and a held
right trigger drive each turn; no score, traffic, AI or player-state is written.

The visually checked Round 1 of 2 result plates show Player 1 at **$17,366** and
Player 2 at **$15,019**. The original intermediate Round 1 of 2 Standings repeats
those values, and normal A advances to the next round. The Round 2 of 2 result
plates show Player 1 at **$15,019** and Player 2 at **$11,150**. Each plate shows
$0 cash bonuses and no multiplier. Final Standings correctly accumulates both
rounds: **Player 1 $32,385**, **Player 2 $26,169**. Fresh normal A presses then
traverse the original Continue/Retry choice and return to Select Option with
Multiplayer selected. OCR recognizes screen readiness; the numbers, round labels
and player names are checked from the actual images independently of OCR.

All 47 actual 640×480 captures decode and match their hashes. The same unchanged
v16 executable runs through the actual RTX 3090 Vulkan 1.4.312 backend, headless
with dummy audio, reading the owned ISO read-only in place. Strict executed-path
checks pass. All five reference and private UDATA/TDATA files remain identical;
no new Party Crash persistence or autosave is claimed.

The input controller exits 0 after capturing Final Standings. The separate
observer exits 143 after the main-menu capture without completing its cleanup;
the cause of that observer exit is untraced. The exact original still-live driver
is verified and given its normal stop-file signal, without restarting the game.
Its log records the normal scene stop at flip 23,446, 490 seconds. The original
driver, shell, engine timeout and native executor are then confirmed absent; an
independent driver exit code is not captured after orphaning. This does not
qualify observer cleanup, window-manager close or app-history handling.

This adds the selected two-round handoff and cumulative-score route. It does not
qualify random-junction selection correctness, other party types/counts,
Crashbreaker/cash/multiplier branches, campaign, exact original-hardware
physics/audio, or physical Windows/Android gameplay. No game rebuild, package
change, immutable Windows preview replacement, stable merge or release occurs.


### v16 four-player Party Crash turn ownership and standings

A separate unchanged-v16 check selects Solo, **four players**, one round, locked
car type Yes and random junctions Yes through the original menus. The selected
junction is Trailer Trash. The four original Get Ready screens identify Player
1, Player 2, Player 3 and Player 4 in order. Each uses the same shared controller,
with normal A followed immediately by a held right trigger for 30,000 ms. All
four turns reach their original finished phase and visibly show Round 1 of 1
Results for the correct player: **$84,633 vehicle damage**, $0 cash bonuses, no
multiplier and **$84,633 Crash $**. No game-state, traffic, AI or score is written.

Final Standings visibly contains all four rows, listing Players 1 through 4
first through fourth with $84,633 each. No Draw label or tie-break rule is
inferred. Normal A traverses the original Continue/Retry choice and returns to
Select Option with Multiplayer selected. All 54 actual 640×480 captures decode
and match their hashes. Numeric values and player names are visually checked
independently of the OCR used to recognize screen readiness.

The first observer exits 143 during Player 2's result loading; the original
finished-phase capture is retained. The game and its exact original driver are
confirmed still live. A detached observer verifies that driver's arguments and
start time, resumes observing the same result, and completes the remaining
turns without a game restart. Its final report is terminal without error. The
normal stop-file signal produces the scene's stopped receipt at flip 21,713,
471 seconds; the exact driver, timeout, native executor and recovery observer
are confirmed absent. The original observer's exit cause remains untraced, and
an independent driver exit code is not captured after its original parent ends.

Strict executed-path and actual RTX 3090 Vulkan 1.4.312 checks pass, headless
with dummy audio and the owned ISO read-only in place. The executable and all
five reference/private UDATA/TDATA files remain unchanged. This qualifies the
selected four-player turn/standings route, not saved Party Crash scores,
random-junction correctness, other counts/types, Crashbreaker/cash/multiplier,
campaign, exact hardware physics/audio, app history, window-manager close or
physical Windows/Android gameplay. No game rebuild or package change occurs.

### v16 normal-input Crashbreaker activation

A separate private Linux v16 run now visibly activates Crashbreaker in the
original Trailer Trash Party Crash event. It uses the same native executable,
the owner's ISO read in place, private cloned saves, actual RTX 3090 Vulkan and
dummy audio. Normal Xbox input starts the event and drives to impact. After
the original impact state at flip 5243, A plus left aftertouch and subsequent
B presses reach the title unchanged. The B press at flip 5396 accompanies the
original state transition from `0x400` to `0x8400`; the captured frame at 5404
shows the original **CRASHBREAKER!** banner and explosion. This is actual
activation, beyond the older driving-gate and B-mapping assertions.

All 65 retained frames decode and match their recorded hashes. The driver
exits normally after the owned stop at flip 7340; the executable and all five
private/reference save files remain unchanged. No autopilot or game-state
writes are used. The earlier probe, whose B presses preceded impact, remains
an incomplete attempt rather than an activation pass.

Its Player 1 results show $108,327 vehicle damage and $5,000 cash, totaling
$113,327. A subsequent log audit corrects the initial interpretation of the
results flow: the last pad change precedes the finish, with no pad changes
afterward. The original runtime explicitly documents that the title can reset
its phase to prepare a retry while the results flag stays set. That phase
reset alone does not establish a replay or another turn. The initial driver
lacks explicit results-latch and no-postfinish-input assertions; a shorter
probe adds those checks. This observation qualifies native activation on this
route, not complete Crashbreaker physics, every junction, other platforms or
the full campaign.

A fresh shorter-input run now passes explicit activation-through-results
checks on the same v16 binary. Four post-impact B pulses activate the original
state at flip 5343; frame 5345 visibly shows the explosion and CRASHBREAKER!
banner. The last neutral pad change is at 5360, before the finish at 5793.
There are no further pad changes; the driving gate stays off, and the results
flag remains set through the title's internal retry preparation. Frame 6932
shows Player 1's original results: $90,609 damage plus $5,000 cash, totaling
$95,609, with no multiplier. All 62 retained captures decode and match their
hashes; the owned driver exits normally, and the executable and all five
private/reference save files remain unchanged.

The first shorter-input wrapper failed an incorrect assertion that the raw
phase must never reset after finishing. That failed wrapper is retained; it
is not a game failure or a passing gate. The fresh passing run instead checks
the original results flag, driving gate and absence of post-finish input,
consistent with the runtime's documented results handling. This adds a
repeatable bounded activation/results observation, not new multiplayer
handoffs, exact physics fidelity or complete campaign/platform coverage.

### v16 campaign Preview Lap Bronze and cold record reload

The unchanged v16 native executable now completes the original World Tour
USA Downtown Preview Lap 1, shown as Burning Lap in its results. Normal menus
select the original Sports Type 3 car from a private copy of the earned v14
Bronze profile. The maintainer-only original-title AI drives this timed lap:
one lap in 81.000 seconds, matching the visible 1:21.00 record and original
Bronze award. The result also shows a 665-foot drift and 347-foot oncoming
record. This is debug AI takeover coverage, not manual-player physics parity;
no finish, time, medal, score or unlock is injected.

Normal Continue inputs traverse Burning Lap Results, successful Autosave,
the 245-point breakdown (40 crash bonus plus 205 driving skills), and the
original campaign map. The active profile changes while the executable and
original reference remain unchanged. A fresh v16 process with AI disabled
loads a copy of that exact earned profile. Driver Details shows 3/173 events,
1,780 points and two takedowns; Downtown retains Bronze beside Preview Lap 1
and the prior Road Rage 1. Racing Records / USA / Downtown Southbound displays
the exact saved 1:21.00 lap with the Sports Type 3 car. All five active, snapshot
and reference UDATA/TDATA files remain byte-identical through this cold load.

Both processes exit normally with the actual-Vulkan and strict executed-path
gates passing, dummy audio and the owner's ISO read in place. All 26 retained
frames decode and match their hashes. The fresh-load observer completes its
own bounded normal stop before a subsequent manual stop request; host-level
process checks confirm that both sessions' drivers and observers are absent.
No game-source kit, prepared Rust unit or native binary is rebuilt or changed.
This adds the selected timed-lap/reward/save/reload route, not the remaining
campaign events, exact controls/audio/physics fidelity or physical Windows
gameplay qualification.

### v16 Waterfront Face-Off loss and cold record persistence

The same unchanged Linux x86-64 v16 native executable completes the original
one-lap Waterfront Face-Off with one rival. The maintainer-only original-title AI
drives the Compact car and finishes second of two in 137.017 seconds. The
original loss screen displays 2:17.01, a 456-foot drift and 402-foot oncoming
record. Normal Continue actions reach Face-Off Results (165 points, zero
takedowns), successful Autosave, the points breakdown and the campaign map.
No finish, reward, score, time or unlock is injected.

A fresh process loads a copy of this earned profile with AI disabled. Driver
Details retains 3/173 completed events and two takedowns, while points increase
from 1,780 to 1,945, exactly the earned 165. The Waterfront Face-Off remains
unmedalled and later Muscle Grand Prix / Preview Lap entries remain locked.
The first Racing Records lookup, Waterfront Westbound, has no entry; a second
fresh process checks Waterfront Eastbound and displays the saved 2:17.01 lap
with the Compact car. The empty opposite-direction page is not a missing-save
defect. All five active, snapshot and reference UDATA/TDATA files remain
byte-identical through both cold loads; the original pre-event reference and
executable remain unchanged.

All three drivers exit normally and pass actual-Vulkan and strict executed-path
checks, using dummy audio and the ISO read in place. All 33 retained captures
decode and match their hashes. This qualifies the selected loss/save/record route, not a winning Face-Off, its car-unlock branch,
manual-player physics equivalence, every campaign event or physical Windows.
