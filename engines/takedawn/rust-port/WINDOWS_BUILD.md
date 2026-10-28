# Native Windows prepared-source build

The standalone Windows pipeline compiles every prepared Rust game unit, builds
the original C/C++ runtime and produces a native Windows executable. It invokes
no WSL, translation tool, Python or game code during compilation. Python and Wine
are maintainer diagnostics. The app's Windows Build action now dispatches this
pipeline for the reviewed v15 kit and older admitted recipes. Windows CLI/GUI Play now uses the owned,
confined session with remembered ISO selection and play history. The current
unreleased 0.1.4 port recipe selects v15; the host also admits the older reviewed Linux
v8 and universal v12/v13/v14 kits for package continuity. The player stops on any
reported unsupported execution; older recipes may need updating. Physical Windows gameplay qualification and stable
release promotion remain unfinished.

## Managed SDK setup

On 64-bit Windows, run `gamepkg setup-native-tools`, or choose **Prepare native
build tools** in Settings. Setup downloads about 485 MB and uses about 2.4 GB
including cached archives. No vendor installer, Python, WSL or administrator
installation is required. The downloader selects Windows' system curl directly,
so an executable in the current library directory cannot shadow it. Setup also
compiles the three host-owned build helpers with the downloaded native Rust
compiler and its bundled linker. Verified helpers are cached separately from
the SDK, with source/SDK identity and stable paths for resumable unit builds.

`src/takedawn_windows_sdk.rs` installs the official standalone tools. Rust 1.89.0
GNU components have gzip pins from Rust's published channel metadata; LLVM-MinGW
20260922, CMake 3.31.12, Ninja 1.13.2, SDL2, Vulkan headers and glslang are pinned in
`../windows-dependencies.json`. Extraction verifies each archive before reading
its payload. Rust component notices are retained; no install script executes.

The expected payload inventory is independently derived from those official
archives: **21,459 files, 1,858,144,785 bytes**, canonical inventory SHA-256
`c26f09324eda2a9f8091bf7330d5363bd22f4cee9a98fb93fe58bbe3eb02ec67`.
Every cached file is checked on reuse. Editing both a binary and its local cache
manifest still fails the host's inventory pin. Bad caches are preserved under a
quarantine name and replaced atomically. Links, special files, path escapes,
Windows device names and case aliases are rejected. GitHub's PAX commit comments
are metadata only; path overrides are not accepted.

Six local fixture tests pass, including cache reuse/repair, forged cache metadata,
archive conflicts, links and path aliases. The real installer also passes with
all official archives and the pinned full inventory. The resulting SDK builds
the complete native game/runtime under Wine. Actual Windows app CI separately
checks the CLI installation, reuse, every payload hash and native tool startup
before its legacy WSL fixtures. That managed SDK setup gate passes on actual
Windows; it does not establish full original-game Build or playback.

## Build tools

`build-windows-units.rs` accepts host-supplied paths to an admitted prepared tree,
verified SDK, output/cache directories and the CPU choice (`native` or `x86-64`).
Inputs are `units.txt`, `exports.txt` and `src/`. It compiles at optimization level
3, validates resumable COFF checkpoints, assembles the native archive with Rust's
standard library and checks every required export before writing
`qualification.json`.

`build-windows-runtime.rs` accepts `--kit`, `--sdk`, `--game-archive`, `--output`,
`--cpu` and `--jobs`. It builds the media bridge offline from vendored sources,
compiles glslang, derives Vulkan loader imports from pinned headers, and builds
the original runtime through native CMake/Ninja/Clang. It retains Vulkan, audio,
input, original settings and managed restart. It copies SDL2.dll beside the game
and records source, archive, executable, companion and tool-version hashes.
A failed build removes any prior qualification receipt. The caller still owns
source-kit/SDK admission, final integrity publication and launch. The app now
performs admission and publication through `src/takedawn/windows_build.rs`:
it verifies a signed package snapshot, rejects the older Linux-only kit on
Windows, prepares tools, invokes both builders and rechecks the installed package
before publishing. SDL2 must match the admitted SDK; executable and companion
integrity receipts are checked when inspecting the native cache. Cache identity
includes the Windows tool recipe, helper sources, CPU model/features and OS AVX
support. Windows PE artifacts are checked for architecture and executable type.
The previous Linux recipe/cache behavior is preserved, and its source loader
also admits the reviewed v12 and v13 kits. The source loader retains the exact
admitted `kit.json` after every payload has verified, so the runtime builder
receives the same manifest that the host checked. Compiler children do not open
console windows.

The game and renderer retain native CPU optimization. The host I/O bridge uses
the portable x64 CPU target with release/LTO optimization: CPU-specific flags in
that library reproducibly missed the original audio deadline. The preceding
library, a replacement of that library alone, and a native-built generic-CPU
library passed. The corrected full native build passes the unchanged check.

Checkpoint identity includes compiler/CPU features, model/vendor, wrapper bytes,
export inventory, source bytes and compiler options. Windows SHA-256 uses BCrypt.
ELF and normal/BigObj COFF objects are validated. Explicit path remapping permits
verified reuse across private source directories. Linux retains its byte-identical
`rustc-cache-linux-v1.rs` so this work does not invalidate existing Linux cache
identity.

## Qualification

The **Full native Windows game package** workflow performs the complete app
Build path on a disposable Windows runner. Its current source-kit pin is
`7f56741fa8f0a56d3e718af1f772bae9200514b6293c2d218e56cde700123c8b`
(v15, recipe 0.1.4). It signs and installs the real source package with a
temporary key, builds all 487 prepared Rust units with native optimization,
and checks readiness and integrity. It damages the private SDL companion and
repairs it through the normal app action, retaining every object hash and
modification time and the unchanged signed package.

[Run 40612591810](https://github.com/centrexity/GamePKG/actions/runs/40612591810)
passes this current v15 gate on actual Windows: cold Build takes about 4 hours
26 minutes on that runner and repair about 112 seconds. It uses no original
media and does not establish gameplay or physical graphics/audio/controllers.
The earlier v14 [run 40446319498](https://github.com/centrexity/GamePKG/actions/runs/40446319498)
remains a historical 7,517-second Build/74-second repair measurement with its
own kit pin. The older v13 result below is separate too.

The separate **Confined native Windows window** gate runs the production owner
with an exact synthetic-media fixture. It creates, shows, pumps and destroys
real Win32 windows through two managed-restart launches and a default-watchdog
launch. [Run 40456563923](https://github.com/centrexity/GamePKG/actions/runs/40456563923)
passes all assertions on native Windows, including the existing save/history and
unsupported-path cases. No ISO, WSL or Python is needed. This verifies basic
window access under the actual confinement, not SDL/Vulkan or physical devices.

Run the same maintainer gate on 64-bit Windows with PowerShell 7:

```powershell
./tools/check_takedawn_windows_package.ps1 -Gamepkg /path/to/gamepkg.exe -Work /new/private/evidence-folder
```

The evidence folder must not already exist. The gate uses no ROM, WSL or Python;
only the verified SDK and Windows system tools are discoverable by the app.
The first build can take hours. CI retains logs and result metadata only, deletes
the temporary signing key, and does not upload game executables or the library.
This gate verifies compilation and repair, not original-game playback. Earlier
full runs compiled all prepared units but exposed a missing extracted `kit.json`
at runtime handoff. The loader now retains that admitted manifest. The next
[full run](https://github.com/centrexity/GamePKG/actions/runs/40204262860) got past
that handoff and built the media bridge, then failed when CMake preserved an
extended source path in glslang includes containing `..`.

The host now normalizes CMake source/build arguments as well as `-D` paths,
while retaining extended executable paths for deeply nested SDK tools. The
corrected builder compiles the entire native runtime using the actual Windows
SDK under isolated Wine, including a deliberately extended SDK path with spaces.
Its executable retains all 24,032 exports and passes original profile
creation/reload and 45-second non-silent stereo capture with dummy output.
Both the managed SDK gate and the full package gate now compile the real shader
library through the production configuration before attempting the expensive
game units. Three argument/policy tests and the real shader test pass locally;
the latter runs the Windows SDK under Wine with canonical extended paths.

The replacement [full run](https://github.com/centrexity/GamePKG/actions/runs/40229229052)
is active. Its complete app Build/repair result remains pending; the successful
Wine runtime build does not substitute for actual Windows qualification.

[40054185588](https://github.com/centrexity/GamePKG/actions/runs/40054185588)
passes actual MSVC32/MSVC64 cache/hash/BigObj, media, file/stream/kernel and strict
process/restart checks. Its x64 standalone SDK gate compiles both Rust builders,
reuses real Rust checkpoints, and builds/runs C++/Rust heap allocation through
native CMake/Ninja/Clang. Linux checks also pass.

The complete **486-unit, 24,032-export** prepared game archive now compiles with
standalone native Windows Rust and `target-cpu=native` under isolated Wine. The
resumed build preserved its verified checkpoints and completed in 5,806 seconds.
Archive SHA-256 is
`2b240179185bfcc3490decbf711e188480b0a9440fd68689482eba59bbf46c21`.

Using GamePKG's managed SDK and that archive, the native runtime builder produces
the v6 executable: **60,064,768 bytes**, SHA-256
`c46120afab1ed10989ea0b9013cc30d45a9b6e98e7ac9b530948d48dd8576fc0`.
All game and runtime compilation uses Windows tools; no Linux cross compiler
supplies this build. It passes these original-engine checks under Wine:

- Xbox race controls, acceleration (+10.4 m/s), pause/resume and Quit.
- PS2-layout Crash controls, impact, results and Continue.
- Original profile creation/reload with unchanged 59,904-byte profile and 54-byte
  metadata, plus 45 seconds of non-silent 48 kHz stereo capture.
- Original settings UI changes render scale from 1x to 2x, cancels then confirms
  restart, verifies the persisted setting without a pending-change prompt and
  quits normally. One Rust session admits the read-only ISO twice despite an
  invalid legacy filename, opens two Vulkan windows and completes both original
  shutdowns. Renderer-owned captures are retained. The owner event is authoritative
  when a late C-stdio teardown diagnostic is unavailable.

Earlier baseline-CPU v5 Crash testing hit its diagnostic deadline; it is not
counted as a pass. The final native-CPU v6 build passes the original gate without
weakening its input, result or timeout assertions. Parallel Wine timing failures
are likewise retained as unqualified diagnostics.

## Source-kit candidate and remaining gates

Private v12 contains the fixed runtime builder and prepared Rust. Archive SHA-256:
`ab30c4e7a4a86580a6fbefe7f79be941c713a0dd2b089d73fba6180729ab845b`
(55,265,670 compressed bytes; 4,257 inventoried files). The actual Rust loader
accepts it, all source hashes remain intact after the full Windows build/tests,
and its prepared Rust/Linux runtime sources match the earlier qualified Linux
candidate. The v6 build uses this exact source tree and its complete native-CPU
archive. Older SDK metadata in this frozen kit is informational; the app admits
managed tools against its current host-pinned inventory.

The app Build/Play dispatch passes Windows cross-target compilation/lint and
local readiness/path/artifact fixtures. Windows Play snapshots the verified
executable and SDL companion, opens the selected ISO in place, locks saves by
game/creator/schema and uses a per-game AppContainer without network capabilities.
The same process owner retains its container, disc and original save path across
normal restarts. Play time begins after disc admission and window readiness, is
checkpointed by the host and is flushed on exit. A restart gets a fresh readiness
deadline. CLI and GUI dispatch this path; no GamePKG game-window shell is opened.

Windows Play now applies the profile's 4 GiB committed-memory job limit before
execution, including across managed restart. Native 32/64-bit allocation probes
and the exact 4 GiB configuration check pass in
[run 40232579957](https://github.com/centrexity/GamePKG/actions/runs/40232579957).
The new memory-configured owner also passes original settings 1x-to-2x restart
and normal Vulkan-window close under Wine. Wine is compatibility evidence;
actual Windows tests establish allocation enforcement. Original-game Windows
hardware compatibility and native frame/hang time policy remain open.

The new Windows app lifecycle fixture passes native Windows execution in job
[120782362776](https://github.com/centrexity/GamePKG/actions/runs/40063096197/job/120782362776),
including confined restart, original save retention, exclusive ownership,
companion integrity and history. All 333 library tests pass (nine optional gates
ignored). The broader job subsequently fails three unrelated Arkanoo native-CRT
pixel comparisons; that run is not an overall pass. The lifecycle fixture does
not exercise Vulkan or original game code. A subsequent
[Windows 32/64-bit run](https://github.com/centrexity/GamePKG/actions/runs/40211935157)
also creates an owned native window inside the confined child, verifies its
painted pixels, resizes/repaints it and closes it before and after managed
restart. This establishes basic confined window access, not Vulkan access.
Complete signed-package Windows Build/Play, container Vulkan access and stable
release promotion remain open. Actual
Windows hardware gameplay, complete campaign progression, physical controllers
and audible/steering fidelity still need evidence. Wine and narrow API fixtures
do not establish those scopes. No stable release is promoted by these checks.

## Current v13 Wine verification

The current v13 runtime executable has SHA-256
`4b19dfdf55487125225b5c35851aa168949ec32b7f0a4678bbc1a41d1f044fb8`.
It combines the complete Windows-Rust archive above with the current original
runtime and reviewed input-glyph adaptation. This runtime was cross-linked on
Linux, separately from the full normal Windows app Build gate.

Private Wine checks pass keyboard profile navigation, a visible Vulkan window,
1024-by-768 resize and normal close. The current Rust session owner also passes
an original settings change from 1x to 2x, cancellation of the first restart
confirmation, confirmed restart, two ISO admissions, two Vulkan windows and two
normal shutdowns within the same tracked session. The 2x setting persists.
Original profile creation/reload preserves both save-file hashes; 45 seconds of
non-silent 48 kHz stereo capture passes without reported underruns in that run.
The ISO is read in place and only disposable save folders are used. These results
do not qualify actual Windows Vulkan access, physical controllers or audible
fidelity. The current v13 executable also passes the original PS2-layout Crash
control assertions under Wine: acceleration, impact/aftertouch/Crashbreaker,
results and Continue back to the front end. The unchanged checker reports the
layout active only during driving; title/menu controls keep their original map.
The separate PS2-layout Race check passes acceleration, braking, pause/resume,
right-trigger mapping and normal Quit/Yes back to the front end.

The packaged Windows CLI and GUI from native host CI also pass relocated version
and embedded-audio checks under Wine. The signed v13 source package verifies,
installs into a disposable library and appears in the library listing. These
smoke checks do not substitute for the full app Build/repair gate. The packaged
GUI also opens its library with the default Vulkan renderer, displays the private
installed recipe, resizes to 800 by 600 and closes normally under Wine.

The same v13 Windows executable completes the original three-lap race under Wine:
second of six, 315.817 seconds, best lap 102.050 seconds, original result screen.
The original AI/menu driver and gameplay checker are unchanged. This is one
bounded event, not full campaign coverage.

A reusable silent package check needs no ISO:

```sh
python3 tools/check_windows_wine_package.py /path/to/gamepkg-VERSION-windows-x86_64.zip --gui --drop
```

It verifies the adjacent checksum and executable metadata, initializes a fresh
Wine prefix fully, checks both embedded audio entries, signs/installs/lists the
collect-stars fixture and exercises the actual library window. Networking and
app update checks are disabled, audio is muted/dummy, and only its unique window
is targeted. Evidence stays in a disposable directory; the temporary creator key
is removed. A prior diagnostic prefix had missing COM proxy registrations and
failed `RegisterDragDrop` with `E_NOINTERFACE`; Wine's own initialization fixed it
without changes to the packaged application. This is a test-environment result,
not a production application fix. The optional `--drop` check compiles
`tests/fixtures/windows-ole-drop-source.c` with maintainer MinGW tools. That source
launches the GUI in the same Wine instance and uses real Windows `DoDragDrop`/
`CF_HDROP`, rather than inserting an egui event. A fresh-prefix run passes the OLE
copy operation, normal GUI close and independent CLI confirmation that the
catalog entry persists with no downloaded package. The test releases its mouse
button even on failure and targets only its uniquely named window. A first
private harness attempt used separate Wine PID/tmp namespaces and could not
exchange the marshalled window; it is retained as a harness failure, not a
passing drop test or an application defect.

## Normal signed Linux v12 candidate

A private v0.1.1 package now passes normal signature verification, install, native
Build and integrity. Its 66,783,496-byte executable has SHA-256
`cf40fb68f443dff431c23bf1a1c1df95d6010b75fa2ecad926d2a0db510dda02`
and retains all 24,032 exports. All 486 game objects are byte-identical to the
qualified v9 inputs. Normal Run rejects an incorrect disc without remembering it
or recording play time, accepts the selected original ISO, remembers it for the
next run and increases host history; no ISO is copied into the library.

The exact executable passes Xbox race (+11.1 m/s), pause/resume/Quit, PS2-layout
Crash (+49.3 m/s), impact/results/Continue, original profile creation/reload and
45 seconds of non-silent 48 kHz stereo capture. Two preceding audio attempts
missed the unchanged capture deadline (28 and 43 seconds captured); the prior
qualified executable passes the same control. A subsequent observed run of the
unchanged new executable passes. These timing failures remain recorded and
real-time audio consistency is not qualified by the successful run. Neither
checker assertions nor deadlines were relaxed. No stable package is promoted.
