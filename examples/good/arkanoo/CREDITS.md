# Arkanoo for GamePKG

Adapted from c0m4r/arkanoo: https://github.com/c0m4r/arkanoo
Upstream revision: 9d613e78c9ed5657ae7fda9206cd4a82a893b7c8
Copyright (C) 2025 c0m4r. Upstream code: WTFPL version 2.
Upstream credits: c0m4r, Gemini 3 Pro, Claude Sonnet/Opus 4.5.

This port adapts the nine campaign layout predicates from entities.rs,
paddle/ball motion and spin, overlap collision response, scoring, and the
long-paddle, extra-ball, ghost-ball and rocket mechanics from entities.rs and
game.rs. The engine integration has been rewritten for GamePKG's safe APIs.

Rendering, bitmap lettering and synthesized effects are new; upstream Pixabay
music/sound, fonts, logos and generated background art are not redistributed.
The port uses a 640x480 canvas, bounded particles/balls/bonuses, swept substeps,
and a deterministic local generator instead of SDL2, rand or filesystem access.
Endless levels use new bounded procedural patterns. The upstream editor,
mouse controls, menus, portal/gravity effects and music are not included.

Controls: Left/Right or A/D move; Up/W launches, fires a rocket, and confirms
start/next level/restart; Down/S pauses/resumes. Three lives, nine campaign
levels, then endless play. High score and unlocked level persist in the game's
scoped save. Active play state stays inside this game's own Wasm memory.
