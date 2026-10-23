# Native Windows process and restart ownership

This host Rust library owns native Windows Takedawn processes and restarts.
The app's Build/Play actions, save/history integration and native builder still
need connecting. This library introduces no game dependency, Python or WSL.

`Process` transfers a duplicate of the caller's opened ISO with read access,
a write-only runtime log and NUL stdin through an explicit
`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`. A suspended child joins an owned
kill-on-close job before executing. Original handles and their inheritance flags
remain the caller's. Dropping the owner stops its process tree. Polling reports
exact exit codes, including 259, separately from a running process. Arguments and
the explicit environment use Unicode, without a command shell or executable search.

`Session` retains one `Launch` across restarts, including its owned ISO file,
arguments, environment, working directory and log. It additionally transfers a
private event with only permission to signal it. The pinned original runtime's
restart exit handler signals that event and returns, allowing every remaining
exit handler and CRT cleanup to run normally, including handlers registered
before `main`. The parent waits for exit zero and all descendants to finish,
then launches again with fresh inherited handles. It never reopens the ISO by
filename. `Event::Restarted` lets the app reset readiness observations while its
session clock and save ownership continue. A nonzero exit after a signal is
reported as an error exit; a failed relaunch remains terminal without hidden
retries or a panic on later polling.

`../media-bridge/adapt_windows_restart.cmake` stages the exact pinned upstream
restart source, preserving request, cancellation, quit and teardown logic.
Staging reverses byte-for-byte and rejects changed/in-place inputs. An absent
managed event retains upstream's standalone behavior. A broken managed signal
logs a diagnostic and closes instead of creating an untracked process.

Process ownership is separate from filesystem/network confinement and signed
package admission. Those app responsibilities, normal launch/history integration,
the native builder and player-kit promotion remain required. No extra game
memory or execution deadlines are introduced here.

Native validation passes on both MSVC architectures in
[40039958125](https://github.com/centrexity/GamePKG/actions/runs/40039958125):

- Eight process tests cover read-only ISO access, retained caller ownership,
  unrelated inheritable-handle exclusion by kernel-object identity, quoted and
  Unicode arguments, exact exit codes, child/descendant termination, restart
  after ISO pathname replacement, error exits and failed relaunches.
- An explicitly invoked integration fixture runs the original upstream restart
  code through `Session`. It verifies one restart after all cleanup, including
  a handler registered before upstream's own, and continued exclusive save access.
- Three native lifecycle cases also verify request, cancellation and ordinary
  quit with the actual event protocol. The default suite explicitly ignores the
  integration fixture until this compiled pinned fixture is supplied.

A 32-bit process-layer check does not qualify the x86-64 game for 32-bit Windows.
Microsoft documents the explicit
[handle list](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute),
[process startup](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw)
and [job assignment](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-assignprocesstojobobject).

## Full native game check

`examples/native_game.rs` is a maintainer probe using the same session API. Its
arguments are absolute executable, owned ISO, private saves, log, `--headless`
or `--window`, and explicit probe seconds (1–300). Only the probe has a deadline;
its intentional stop is separate from normal game completion. It uses dummy audio.

The complete updated native executable retains all 24,032 Rust exports in
59,883,520 bytes. Its game source remains the same 486 prepared Rust units; build
commands compile no generated C game bodies and invoke no Python.

A fresh-prefix Wine test uses the original overlay's SDL event driver to change
render scale from 1x to 2x, cancel the first restart confirmation, confirm the
restart and verify 2x remains applied afterward with no pending-change prompt.
The same Rust session admits the ISO twice despite an invalid legacy filename,
opens two Vulkan windows on the RTX 3090, performs both normal shutdowns and exits
zero after the original Quit action. Renderer-owned captures show the settings
before and after. No global keyboard input or game-state edits are used.

Maintainer reproduction, after building the native runner example:

```sh
python3 tools/check_takedawn_windows_restart_window.py \
  --runner /path/to/native_game.exe --game /path/to/game.exe \
  --iso /path/to/owned.iso --saves /path/to/private/reference-saves
```

Python and Wine are diagnostic tools, not player requirements. These results
qualify this Wine window/settings/restart flow, not full campaign progression,
physical controllers, audible fidelity or actual Windows hardware gameplay.
The earlier native executable's race, Crash and profile/audio checks are in the
[media bridge notes](../media-bridge/README.md). The verified Linux player kit
remains unchanged.
