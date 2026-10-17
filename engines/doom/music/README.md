# In-memory DOOM music

The music sequencer, MUS converter, MIDI parser and Nuked OPL3 synthesizer are
pinned from Chocolate DOOM 3.1.0, commit
`35fb1372d10756ca27eca05665bd8a7cebc71c05`. See
[upstream-lock.json](upstream-lock.json) for file hashes and the original
[source](https://github.com/chocolate-doom/chocolate-doom/tree/35fb1372d10756ca27eca05665bd8a7cebc71c05).
Copyright id Software, Simon Howard and Alexey Khokholov; GPL-2.0-or-later.
The unchanged sources and complete license remain in `upstream/`.

`music_opl.c` and `midi_memory.c` are reviewed adaptations. They replace temporary
MIDI files with memory streams, validate instrument and MIDI bounds, reject
out-of-range channel bytes and zero timing divisions, and ignore zero tempo
values. `music_mus.c` compiles the unchanged MUS converter with compatible types
and endian operations. The original instrument definitions and song data come
from the player's selected WAD; no instruments or music are bundled.

`opl_memory.c` supplies a synchronous software-only OPL backend at 48 kHz. It
uses the original sequencer's OPL2 defaults and voice allocation. It renders
800 stereo frames per host tick, mixes with sound effects using signed clipping,
and never accesses a device, wall clock, filesystem, environment or thread.
The callback queue has 128 entries, dispatches at most 4,096 events per sample,
and freezes its timeline while paused. Tempo changes adjust pending callbacks.
There is no instruction metering. Existing engine deadlines and memory limits
remain in force.

Music input is limited to 96 KiB, 64 MIDI tracks and 65,536 events per track.
Track byte ranges and variable-length data must remain within the supplied
memory. Pinned source hashes are checked before native and Wasm compilation.
Game packages cannot replace this C source or add dependencies.

`opl_synth.c` preserves the pinned emulator with one reviewed arithmetic fix:
its attack-envelope update uses defined multiplication/division instead of
left-shifting a negative signed value. Original bytes remain in `upstream/`.
The first three seconds of all 45 music lumps in the selected DOOM WAD pass
address and undefined-behavior sanitizers through `tests/doom_music_wad.c`.

`tests/doom_music.c` tests actual synthesis from generated MIDI and MUS,
volume reduction/restoration, looping, pause/resume timing, callback limits,
truncations and every single-byte MIDI mutation. It uses generated instrument
fixtures and runs with address, undefined-behavior and leak sanitizers.
`tests/doom_play.rs` checks original WAD title music before demo effects,
spatial sound in the original attract demo, gameplay and save/reload on both
native and Wasm targets. Full campaigns and every WAD song are separate work;
these checks do not prove them complete.

Native cache identity includes the embedded DOOM and Zelda source kits. Engine
fixes therefore require a rebuild even if the signed package and host version
have not changed; the GUI reports that a build is needed.

Sound settings now persist through the original configuration files and survive
relaunch. Optional music-backend/enhancement selection and the remaining port
gaps stay tracked in [port-parity.toml](../../../docs/port-parity.toml).
