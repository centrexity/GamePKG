#!/usr/bin/env bash
# Maintainer-only gameplay gate using the pinned upstream control assertions.
# Python belongs to the upstream checker, not the player build or runtime.
# GAMEPKG_GAMEPLAY_WINE=1 runs the Windows executable with the same assertions
# in an isolated private Wine prefix. It does not qualify actual Windows.
# race-complete uses the original AI driver through a whole race and results;
# the race/crash modes retain the separate original player-control assertions.
set -euo pipefail
if [[ $# -lt 4 || $# -gt 6 ]]; then
    echo "usage: bash $0 NATIVE_EXECUTABLE ISO PINNED_TAKEDAWN PRIVATE_REFERENCE_SAVES [xbox|ps2] [race|crash|race-complete]" >&2
    exit 2
fi
for task_tool in bwrap systemd-run timeout realpath sha256sum awk python3; do
    command -v "$task_tool" >/dev/null || { echo "missing maintainer tool: $task_tool" >&2; exit 2; }
done
task_binary=$(realpath -- "$1")
task_disc=$(realpath -- "$2")
task_source=$(realpath -- "$3")
task_saves=$(realpath -- "$4")
task_scheme=${5:-xbox}
task_event=${6:-race}
task_wine=${GAMEPKG_GAMEPLAY_WINE:-0}
[[ "$task_wine" == 0 || "$task_wine" == 1 ]] || exit 2
if [[ "$task_wine" == 1 ]]; then
    command -v wine >/dev/null || { echo "missing maintainer Wine" >&2; exit 2; }
    [[ -n "${DISPLAY:-}" && -f "${XAUTHORITY:-}" && -f "$(dirname -- "$task_binary")/SDL2.dll" ]] || {
        echo "Windows Vulkan checks require DISPLAY, XAUTHORITY and SDL2.dll beside the executable" >&2; exit 2;
    }
    export GAMEPKG_WINE_DISPLAY="$DISPLAY" GAMEPKG_WINE_XAUTHORITY="$XAUTHORITY"
    export GAMEPKG_GAMEPLAY_WINE=1 GAME_PATH_PREFIX=Z:
fi
[[ -x "$task_binary" && -f "$task_disc" && -d "$task_saves" ]] || exit 2
[[ "$task_scheme" == xbox || "$task_scheme" == ps2 ]] || exit 2
[[ "$task_event" == race || "$task_event" == crash || "$task_event" == race-complete ]] || exit 2
task_timeout=300
task_driver=controls_scene.sh
if [[ "$task_event" == race-complete ]]; then
    [[ "$task_scheme" == xbox ]] || { echo "race-complete uses the original AI driver, not a PS2 control-layout check" >&2; exit 2; }
    task_timeout=1200
    task_driver=race_drive.sh
fi
# Fail on changed drivers rather than silently adapting a different upstream test.
for task_spec in \
    '86ec5ad1eecea98ac3f090d995f7c64696210c9a1763e1c253209942e5709c1b controls_scene.sh' \
    'e7f2bbd7332bb892898e4524668c21a41aa4e183877a5c14d6c689d7ce7a67f2 scene_lib.sh' \
    '2b299c0571be0feddeb1c241eb0610e2f0bb7f91124419a56f7d233978b82b8a controls_check.py'; do
    read -r task_hash task_file <<< "$task_spec"
    [[ $(sha256sum "$task_source/tools/headless/$task_file" | cut -d' ' -f1) == "$task_hash" ]] || {
        echo "unreviewed upstream gameplay driver: $task_file" >&2; exit 2;
    }
done
if [[ "$task_event" == race-complete ]]; then
    [[ $(sha256sum "$task_source/tools/headless/race_drive.sh" | cut -d' ' -f1) == 3d13d2fb92769a641e4e1d536b966104f4af4c5b14ae702935410a1c8c762b93 ]] || {
        echo "unreviewed upstream full-race driver" >&2; exit 2;
    }
fi
task_state=$(mktemp -d /tmp/gamepkg-takedawn-gameplay.XXXXXXXX)
export GAMEPKG_SCENE_STATE="$task_state" GAMEPKG_GAMEPLAY_BINARY="$task_binary" GAMEPKG_GAMEPLAY_DISC="$task_disc"
export GAMEPKG_GAMEPLAY_TIMEOUT="$task_timeout" GAMEPKG_GAMEPLAY_EVENT="$task_event"
mkdir -p "$task_state"/{game,home,data,cache,driver/tools/headless}
echo "Private evidence directory: $task_state"
cp "$task_source/tools/headless/"{scene_lib.sh,controls_check.py} "$task_state/driver/tools/headless/"
# The upstream event filter can lag its direct control-state log by a second.
# Its frame-relative waits must start at the current rendered frame, otherwise
# a 60-frame menu wait can become 10 frames and drop the first navigation press.
# Read only a bounded tail of the same original log and retain the filtered
# counter as a fallback. Inputs, game state and checker assertions are unchanged.
cat >> "$task_state/driver/tools/headless/scene_lib.sh" <<'CLOCK'

eval "scene_filtered_flip() $(declare -f flip | tail -n +2)"
flip() {
    local direct filtered
    direct=$(tail -c 65536 "$D/log.txt" 2>/dev/null | awk '/\[GPU\] flip [0-9]+ / { n=$3 } END { print n+0 }')
    filtered=$(scene_filtered_flip)
    if (( direct > filtered )); then echo "$direct"; else echo "$filtered"; fi
}
CLOCK
# Use a real Start press to skip the tutorial. No game state, engine feature,
# checker assertion, driving input or result expectation is replaced.
awk '
    /wait_log '\''streamed/ {
        if ($0 ~ /junction/) print "    wait_log '\''TrC_ENG[.]xmv'\'' \"crash tutorial\""
        else print "    wait_log '\''TrD_ENG[.]xmv'\'' \"driving tutorial\""
        print "    wait_flips 600"
        print "    press start:300 0"
        n++
    }
    { print }
    END { if (n != 2) exit 1 }
' "$task_source/tools/headless/controls_scene.sh" > "$task_state/driver/tools/headless/controls_scene.sh"
if [[ "$task_event" == race-complete ]]; then
    # Keep the complete original AI/race/results flow, without changing game
    # state, unlocking tracks, shortening laps or replacing the finish check.
    cp "$task_source/tools/headless/race_drive.sh" "$task_state/driver/tools/headless/race_drive.sh"
fi
cat > "$task_state/launch.sh" <<'LAUNCH'
#!/usr/bin/env bash
set -euo pipefail
task_state=$GAMEPKG_SCENE_STATE
[[ "$task_state" == /tmp/gamepkg-takedawn-gameplay.* && -d "$task_state/game" ]]
task_disc_path=/disc.iso
[[ "${GAMEPKG_GAMEPLAY_WINE:-0}" == 1 ]] && task_disc_path='Z:\disc.iso'
task_args=(--die-with-parent --unshare-net --clearenv
    --ro-bind /usr /usr --ro-bind /lib /lib --ro-bind /lib64 /lib64
    --ro-bind /etc /etc --ro-bind /sys /sys --dev-bind /dev /dev --proc /proc --tmpfs /tmp
    --bind "$task_state" "$task_state" --chdir "$task_state/game"
    --ro-bind "$GAMEPKG_GAMEPLAY_BINARY" /engine --ro-bind "$GAMEPKG_GAMEPLAY_DISC" /disc.iso
    --setenv PATH /usr/bin:/bin --setenv LANG C.UTF-8 --setenv HOME "$task_state/home"
    --setenv XDG_DATA_HOME "$task_state/data" --setenv XDG_CACHE_HOME "$task_state/cache"
    --setenv GAMEPKG_TAKEDAWN_DISC "$task_disc_path" --setenv RECOMP_PB_BACKEND vulkan
    --setenv RECOMP_STUB_LOG 1
    --setenv SDL_AUDIODRIVER dummy --setenv RECOMP_FB_DUMP "$task_state/run/frame-")
if [[ "$GAMEPKG_GAMEPLAY_EVENT" == race-complete ]]; then
    task_args+=(--setenv RECOMP_DEBUG autopilot=1)
fi
for task_key in RECOMP_HEADLESS RECOMP_PAD_LIVE RECOMP_STDIO_LOG RECOMP_FLIP_MARK RECOMP_SAVE_DIR \
    RECOMP_RENDER_SHADER_CACHE BO3_CTL_LOG BO3_DRIVE_LOG BO3_CTL_SCRIPTED \
    RECOMP_INPUT_CONTROL_SCHEME RECOMP_INPUT_FACE_LAYOUT; do
    if [[ -v "$task_key" ]]; then task_args+=(--setenv "$task_key" "${!task_key}"); fi
done
task_command=(/engine --headless /gamepkg-disc/default.xbe)
if [[ "${GAMEPKG_GAMEPLAY_WINE:-0}" == 1 ]]; then
    task_args+=(--unshare-pid --symlink usr/bin /bin
        --ro-bind /tmp/.X11-unix /tmp/.X11-unix
        --ro-bind "$GAMEPKG_WINE_XAUTHORITY" /xauth
        --ro-bind "$(dirname -- "$GAMEPKG_GAMEPLAY_BINARY")" /engine-dir
        --setenv DISPLAY "$GAMEPKG_WINE_DISPLAY" --setenv XAUTHORITY /xauth
        --setenv WINEPREFIX "$task_state/wine-prefix" --setenv WINEARCH win64
        --setenv WINEDEBUG -all --setenv WINEDLLOVERRIDES 'mscoree,mshtml='
        --setenv RECOMP_FB_DUMP_FLIPS 120)
    task_command=(/usr/bin/wine "/engine-dir/$(basename -- "$GAMEPKG_GAMEPLAY_BINARY")" --headless /gamepkg-disc/default.xbe)
fi
# Physical memory accounting permits the runtime's original RAM aliases and
# GPU reservations. This applies only to this private test process tree.
exec systemd-run --user --scope --quiet --collect --property=MemoryMax=4294967296 \
    --property=MemorySwapMax=0 -- timeout --signal=TERM --kill-after=5s "${GAMEPKG_GAMEPLAY_TIMEOUT}s" \
    bwrap "${task_args[@]}" "${task_command[@]}"
LAUNCH
chmod 700 "$task_state/launch.sh"
export LAUNCH="$task_state/launch.sh" GAME_DIR="$task_state/game" SAVES="$task_saves"
# Golden-frame timing is backend-specific. Retain independent periodic frames
# and every upstream state/input assertion instead of requiring those timings.
export CAPTURE=0 STOP_AT_END=1 SCENE_TIMEOUT=180 RECOMP_PB_BACKEND=vulkan
if [[ "$task_event" == race-complete ]]; then
    export RACES=1 RACE_REGION=1 RACE_TRACK=1 RACE_LIMIT=900
    unset PICK MIX MIX_PLAN EVENTS RACE_STOP RECOMP_DEBUG
fi
task_result=0
SCHEME="$task_scheme" EVENT="$task_event" timeout --signal=TERM --kill-after=5s "${task_timeout}s" \
    bash "$task_state/driver/tools/headless/$task_driver" "$task_state/run" \
    > "$task_state/driver.log" 2>&1 || task_result=$?
if grep -aEq '\[STUB\] unresolved 0x' "$task_state/run/log.txt" "$task_state/run/out.txt" 2>/dev/null; then
    echo "FAIL: gameplay reached an unresolved upstream game-function stub" >> "$task_state/driver.log"
    task_result=1
fi
if grep -aEq '\[UNIMPL\].*(stmxcsr|ldmxcsr)' "$task_state/run/log.txt" "$task_state/run/out.txt" 2>/dev/null; then
    echo "FAIL: gameplay skipped the original native SSE control setup" >> "$task_state/driver.log"
    task_result=1
fi
printf 'exit_code=%s\nscheme=%s\nevent=%s\nnative_sha256=%s\n' \
    "$task_result" "$task_scheme" "$task_event" "$(sha256sum "$task_binary" | cut -d' ' -f1)" \
    > "$task_state/result.txt"
printf 'transport=%s\n' "$([[ "$task_wine" == 1 ]] && echo windows-wine || echo native-linux)" >> "$task_state/result.txt"
# A timed-out driver has not reached its checker. Preserve partial assertion
# results too, without changing the failed exit status or hiding the timeout.
if (( task_result != 0 )) && [[ ! -f "$task_state/run/controls.txt" \
    && -f "$task_state/run/ctl.txt" && -f "$task_state/run/drive.txt" ]]; then
    python3 "$task_state/driver/tools/headless/controls_check.py" "$task_scheme" "$task_event" \
        "$task_state/run/ctl.txt" "$task_state/run/drive.txt" xbox \
        > "$task_state/run/partial-controls.txt" 2>&1 || true
    cat "$task_state/run/partial-controls.txt"
fi
cat "$task_state/driver.log"
if [[ -f "$task_state/run/controls.txt" ]]; then cat "$task_state/run/controls.txt"; fi
if (( task_result != 0 )); then
    echo "FAIL: gameplay gate exited $task_result; evidence retained at $task_state" >&2
    exit "$task_result"
fi
echo "PASS: original native gameplay assertions ($task_scheme, $task_event); evidence: $task_state"
