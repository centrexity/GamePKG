# Zelda3 for GamePKG

Engine: https://github.com/snesrev/zelda3
Pinned revision: 45a149d8b4bb8098f53d1009f76e76e7d8b79bbf.
MIT code copyright 2022 snesrev and 2021 elzo_d; complete notices are included.
GamePKG adaptations are Apache-2.0. Original Zelda game data belongs to its
respective owners and is not distributed with this package.

The restricted Rust entry calls a host-owned, pinned engine compiled into Wasm
under isolation. The engine has no filesystem/network imports. The selected
USA ROM is decoded into temporary RAM resources without extracted asset files.
Only game SRAM is written through the creator-scoped save API.

Arrows/WASD move; Z sword/B; X equipped item/Y; C interact/A; V map/X;
Enter Start/pause; Tab Select/save menu; Q/E L/R.
