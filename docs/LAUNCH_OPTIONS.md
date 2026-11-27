# Game launch options

GamePKG stores per-game launch overrides separately from signed packages. A
game page can expose host-provided choices such as window mode and renderer
settings when the runtime declares a safe adapter for them. Choices are scoped
to the game ID, creator and runtime profile; **Use package defaults** clears the
host override and restores the signed manifest behavior.

Command-line flags follow the same precedence as saved launch options: explicit
flags, saved host options, signed manifest values, then runtime defaults. Use
`gamepkg run --help` and `gamepkg-gui --help` for the available flags and
`gamepkg list` for installed game IDs. GamePKG passes resolved settings as
separate argument/value pairs without a shell. Headless checks ignore window
and graphics options. Executable paths, shell commands, arbitrary arguments and
environment keys are never accepted from a manifest.

Renderer-specific options, supported values and port-side argument readers are
documented by the native engine port in the separate GamePKG Ports checkout.
GamePKG core provides only the generic storage, precedence and validation
contract; it does not ship engine-specific defaults or artwork.
