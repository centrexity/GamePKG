# Native Windows process owner

This host Rust library is the process layer for GamePKG's native Windows
Takedawn integration. It is not connected to the app's Build/Play actions yet.
It introduces no game dependency or package-supplied code, Python or WSL.

The caller retains its opened ISO. Each launch duplicates that handle with only
read access and transfers it using an explicit `PROC_THREAD_ATTRIBUTE_HANDLE_LIST`.
The other inherited handles are a write-only runtime log and NUL stdin. Original
handles and their inheritance flags remain the caller's. A suspended child joins
an owned kill-on-close job before its first instruction executes; dropping the
owner terminates its process tree. Polling distinguishes a genuine exit code 259
from a still-running process. Arguments and the explicit environment use Unicode;
there is no command shell or implicit executable search.

This implements lifetime ownership, not a filesystem/network sandbox or signed
package admission. Those app responsibilities, restart coordination, normal
history/save integration and the native Windows builder remain required before
player promotion. Restart must wait for complete original teardown and launch
again with a fresh inherited duplicate of the parent's retained ISO. Upstream's
current unowned Windows self-spawn cannot carry that handle and must be adapted.
No extra memory or time limits are imposed here; the existing gameplay allowance
is unchanged.

Native tests launch real child executables without game assets. They check
read-only ISO access, the caller's continued ownership, unrelated inheritable
handle exclusion by kernel-object identity, quoted/Unicode arguments, exit code
259 and termination on owner drop. They run on actual MSVC 32/64-bit in the
`Native Rust engine stream ABI` workflow. A 32-bit process-layer test does not
qualify the x86-64 game for 32-bit Windows.

Microsoft's contracts explain the explicit
[handle list](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute),
[process startup](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
and [job assignment](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject).

The first native process fixture run passes all five tests on both MSVC targets
in [40037578848](https://github.com/centrexity/GamePKG/actions/runs/40037578848).
This includes a live descendant process stopped with its owner. The unrelated
handle check captures any existing object before opening its named oracle, so
reuse of an empty numeric handle slot by the oracle cannot produce a false leak.

`examples/native_game.rs` is a maintainer-only, bounded headless probe using the
same Rust API with the full native game. Its arguments are absolute executable,
owned ISO, private save directory, log file, `--headless`, and explicit probe
seconds (1–300). The library itself has no execution deadline. It intentionally
stops the job at the probe's end; that is not normal game completion. The probe
uses dummy audio and does not qualify audible fidelity or window interaction.

The full-game Rust-owner probe passes under isolated Wine with the legacy ISO
filename deliberately invalid: all 716 files verify, the 4,308,992-byte XBE
loads, Vulkan uses the RTX 3090, and 2,375 intro frames present. After the explicit
60-second probe, Rust stops the job and observes child exit code 1; the probe
itself exits zero. This verifies admission, startup, output capture and process
ownership, not a completed race or actual Windows hardware gameplay. The
separate Windows executable race qualification is described in the
[media bridge notes](../media-bridge/README.md).
