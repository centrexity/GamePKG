# Game launch options

The signed manifest can declare `[launch_options]`. GamePKG 0.1.24 supports these options for the reviewed Takedawn native engine on Linux and Windows. Other engines reject the table until their renderer has an explicit adapter. Older packages without this table keep their existing renderer settings.

```toml
[launch_options]
window_mode = "windowed"
aspect_ratio = "16:9"
render_scale = 2
antialiasing = "smaa"
texture_lod = "auto"
sharpening_percent = 10
widescreen_hud = true
menu_hud_scale = 2
```

These are the agreed Burnout defaults: windowed, 16:9, built-in 2× internal resolution, SMAA, automatic texture LOD, 10% sharpening, widescreen HUD correction, and 2× menu/HUD upscaling. Original Xbox button artwork is read from the ISO and upscaled locally by default. Input → Button prompts in the native settings selects other prompt layouts and retains saved choices. Internal resolution uses the original renderer. Menu/HUD 3× extends its existing scaler; authored input prompts are rendered from vectors at each supported scale.

Supported values:

| Field | Values |
| --- | --- |
| `window_mode` | `"windowed"`, `"fullscreen"` (borderless desktop) |
| `aspect_ratio` | `"4:3"`, `"16:9"` |
| `render_scale` | `1`, `2`, `3`, `4` |
| `antialiasing` | `"off"`, `"fxaa"`, `"smaa"` |
| `texture_lod` | `"off"`, `"auto"`, `"-0.5"`, `"-1"`, `"-1.5"` |
| `sharpening_percent` | Integer `0`–`100` |
| `widescreen_hud` | `true`, `false` |
| `menu_hud_scale` | `1` (off), `2`, `3`, `4` |

Missing fields within this table use the defaults above. Put the table after top-level manifest fields and before the next table. Require `engine_version = ">=0.1.24, <0.2.0"` for packages using it.

On a game's page, expand **Launch options**, choose settings and select **Save launch options**. Changes apply on the next native launch and require no rebuild. **Use package defaults** clears the host override and follows the current signed manifest. Choices are stored separately from the signed package, scoped to the game ID, creator and reviewed engine; updates from the same creator preserve them.

Precedence is explicit command-line flags, then saved GamePKG launch options, then manifest launch options, then existing in-game settings and runtime defaults. Host-supplied launch options override the corresponding in-game saved settings, and those controls are locked in the game's overlay for that session. Headless maintainer checks do not apply window/graphics launch options. No executable paths, shell commands, arbitrary arguments or environment keys can be declared here.

Alt+Enter (including keypad Enter and either Alt key) toggles the native game window between windowed and borderless fullscreen. It works with launch options and while the overlay is open. Holding the shortcut toggles once per press. The shortcut does not activate a game menu item. It requires a game rebuilt with the fullscreen-shortcut runtime.

## Command-line launch options

External launchers can use either entry; both launch the separate native game
and retain GamePKG play tracking:

```sh
gamepkg run com.gamepkg.takedawn --fullscreen --aspect-ratio 16:9 --render-scale 3 --antialiasing smaa --texture-lod auto --sharpening-percent 10 --widescreen-hud true --menu-hud-scale 3
gamepkg-gui --play com.gamepkg.takedawn --windowed --render-scale 2
```

Use the installed game ID from `gamepkg list`. On Windows the executable names
are `gamepkg.exe` and `gamepkg-gui.exe`. Run `gamepkg run --help` or
`gamepkg-gui --help` for the available flags. `--window-mode windowed|fullscreen`
is equivalent to the corresponding shortcut flag. Use
`--texture-lod=-1` for an explicit negative LOD bias.

Only supplied flags override the selected settings, for this launch and its
managed restarts. They do not rewrite the manifest or saved launch choices.
GamePKG passes the resolved settings as separate argument/value pairs to the
native executable, without a shell. The native argument reader validates them
before applying the existing renderer controls. Audio, isolation, watchdog and
storage controls remain owned by GamePKG. Graphics flags require the updated
game package and a rebuild; they are rejected for headless runs and other engines.

The native executable accepts the same fixed argument/value pairs before its
disc entry path (for example `--render-scale 3 --menu-hud-scale 3`). GamePKG
supplies the read-only ISO and managed storage; use its entry above for normal
external launchers.
