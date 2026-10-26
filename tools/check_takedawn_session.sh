#!/usr/bin/env bash
# Maintainer-only original-engine save/reload + mixed-audio gate. No extracted
# disc folder, real user library or existing save directory is used.
# GAMEPKG_GAMEPLAY_WINE=1 tests the native Windows build under isolated Wine.
set -euo pipefail
if [[ $# != 2 ]]; then
    echo "usage: bash $0 /path/to/private/burnout3-gamepkg-media /path/to/supported.iso" >&2
    exit 2
fi
for task_tool in bwrap timeout realpath awk grep sha256sum ffprobe ffmpeg find stat mktemp python3; do
    command -v "$task_tool" >/dev/null || { echo "missing maintainer tool: $task_tool" >&2; exit 2; }
done
task_binary=$(realpath -- "$1")
task_disc=$(realpath -- "$2")
task_wine=${GAMEPKG_GAMEPLAY_WINE:-0}
[[ "$task_wine" == 0 || "$task_wine" == 1 ]] || exit 2
if [[ "$task_wine" == 1 ]]; then
    command -v wine >/dev/null || { echo "missing maintainer Wine" >&2; exit 2; }
    [[ -n "${DISPLAY:-}" && -f "${XAUTHORITY:-}" && -f "$(dirname -- "$task_binary")/SDL2.dll" ]] || {
        echo "Windows Vulkan checks require DISPLAY, XAUTHORITY and SDL2.dll beside the executable" >&2; exit 2;
    }
fi
[[ -f "$task_binary" && -x "$task_binary" && -f "$task_disc" ]] || { echo "executable and ISO are required" >&2; exit 2; }
task_state=$(mktemp -d /tmp/gamepkg-takedawn-session.XXXXXXXX)
mkdir -p "$task_state/saves" "$task_state/data" "$task_state/cache"
[[ "$task_wine" == 0 ]] || mkdir -p "$task_state/home"
task_pid=
task_phase=
task_presses=0
task_log=
echo "Private evidence directory: $task_state"

cleanup() {
    if [[ -n "$task_pid" ]]; then
        kill -TERM "$task_pid" 2>/dev/null || true
        wait "$task_pid" 2>/dev/null || true
    fi
}
trap cleanup EXIT
fail() { echo "FAIL: $* (evidence: $task_state)" >&2; exit 1; }
alive() { kill -0 "$task_pid" 2>/dev/null || fail "$task_phase process exited before completion"; }
wait_line() {
    local task_pattern=$1 task_end=$((SECONDS + 45))
    until grep -aEq -- "$task_pattern" "$task_log"; do
        alive
        (( SECONDS < task_end )) || fail "$task_phase did not reach $task_pattern"
        sleep 0.1
    done
}
flip() { awk '/\[GPU\] flip [0-9]+ / { n=$3 } END { print n+0 }' "$task_log"; }
wait_flip() {
    local task_goal=$1 task_end=$((SECONDS + 45))
    while (( $(flip) < task_goal )); do
        alive
        (( SECONDS < task_end )) || fail "$task_phase stalled before flip $task_goal"
        sleep 0.1
    done
}
press() {
    local task_button=$1 task_frames=$2 task_end=$((SECONDS + 45))
    task_presses=$((task_presses + 1))
    printf '%s\n' "$task_button" >> "$task_state/pad.txt"
    until (( $(grep -ac 'PAD: live "' "$task_log" || true) >= task_presses )); do
        alive
        (( SECONDS < task_end )) || fail "$task_phase did not consume $task_button"
        sleep 0.1
    done
    wait_flip "$(( $(flip) + task_frames ))"
}
start() {
    task_phase=$1
    task_presses=0
    task_log="$task_state/$task_phase.log"
    : > "$task_state/pad.txt"
    local task_audio=()
    if [[ "$task_phase" == create ]]; then
        task_audio=(--setenv RECOMP_AUDIO_WAV /state/audio.wav --setenv RECOMP_AUDIO_WAV_SECS 45)
    fi
    # The player's selected disc is the only user file made visible. The
    # renderer's system libraries/devices are present; saves and caches have
    # their own fresh scope. A clean environment prevents inherited secrets or
    # diagnostic switches from changing the tested behavior.
    local task_platform=() task_command=(/engine --headless /gamepkg-disc/default.xbe)
    if [[ "$task_wine" == 1 ]]; then
        task_platform=(--unshare-pid --symlink usr/bin /bin
            --ro-bind /tmp/.X11-unix /tmp/.X11-unix --ro-bind "$XAUTHORITY" /xauth
            --ro-bind "$(dirname -- "$task_binary")" /engine-dir
            --setenv DISPLAY "$DISPLAY" --setenv XAUTHORITY /xauth
            --setenv HOME /state/home --setenv WINEPREFIX /state/wine-prefix --setenv WINEARCH win64
            --setenv WINEDEBUG -all --setenv WINEDLLOVERRIDES 'mscoree,mshtml='
            --setenv GAMEPKG_TAKEDAWN_DISC 'Z:\disc.iso'
            --setenv RECOMP_FB_DUMP_FLIPS 120 --setenv RECOMP_STUB_LOG 1)
        task_command=(/usr/bin/wine "/engine-dir/$(basename -- "$task_binary")" --headless /gamepkg-disc/default.xbe)
    fi
    timeout --signal=TERM --kill-after=5s 100s bwrap --die-with-parent --unshare-net --clearenv \
        --ro-bind /usr /usr --ro-bind /lib /lib --ro-bind /lib64 /lib64 \
        --ro-bind /etc /etc --ro-bind /sys /sys --dev-bind /dev /dev \
        --proc /proc --tmpfs /tmp --bind "$task_state" /state \
        --ro-bind "$task_binary" /engine --ro-bind "$task_disc" /disc.iso --chdir /state \
        --setenv PATH /usr/bin:/bin --setenv LANG C.UTF-8 \
        --setenv GAMEPKG_TAKEDAWN_DISC /disc.iso --setenv RECOMP_SAVE_DIR /state/saves \
        --setenv XDG_DATA_HOME /state/data --setenv XDG_CACHE_HOME /state/cache \
        --setenv RECOMP_PB_BACKEND vulkan --setenv SDL_AUDIODRIVER dummy \
        --setenv RECOMP_FLIP_MARK 1 --setenv RECOMP_PAD_LIVE /state/pad.txt \
        --setenv RECOMP_FB_DUMP "/state/$task_phase-frame-" --setenv RECOMP_PB_REPORT_MS 5000 \
        "${task_audio[@]}" "${task_platform[@]}" "${task_command[@]}" > "$task_log" 2>&1 &
    task_pid=$!
    wait_line 'GamePKG: verified disc admitted in place; 716 files, read-only'
    wait_line 'Titles30[.]xmv'
    until grep -aq '_FEMain[.]xmv' "$task_log"; do press start:300 60; done
    wait_flip "$(( $(flip) + 300 ))"
    press start:600 150
}
stop() {
    alive
    kill -TERM "$task_pid" || fail "unable to stop $task_phase process"
    local task_status=0
    wait "$task_pid" || task_status=$?
    task_pid=
    # SIGTERM is intentional. A natural exit/crash or the external timeout is
    # not accepted as a successful session, even if it wrote some evidence.
    [[ "$task_status" == 143 ]] || fail "$task_phase stop returned $task_status, expected intentional SIGTERM"
}

start create
press a:400 150                       # New Profile -> keyboard, default Done
press a:400 150                       # Done -> save prompt, default No
press up:200 30                       # Yes
press a:400 150                       # choose a profile slot
press a:400 150                       # Profile 1 -> save
wait_line '\[SAVE\] Profile 1: replaced .*59904 bytes'
task_profiles=()
mapfile -d '' task_profiles < <(find "$task_state/saves/UDATA" -type f -name 'Profile 1' -print0)
[[ ${#task_profiles[@]} == 1 ]] || fail "expected one newly created Profile 1"
task_profile=${task_profiles[0]}
task_metadata="${task_profile%/*}/SaveMeta.xbx"
[[ $(stat -c %s -- "$task_profile") == 59904 && $(stat -c %s -- "$task_metadata") == 54 ]] || fail "unexpected save lengths"
sha256sum -- "$task_profile" "$task_metadata" > "$task_state/profile-before.sha256"
press a:400 180                       # Save successful -> main menu
wait_line '\[AUDIO-WAV\] closed: 2160000 samples \(45[.]00 s\)'
stop

start reload
wait_line 'SaveMeta[.]xbx'
press a:400 150                       # Load Profile -> populated slot list
wait_line 'Profile 1 \(disp 1 '
wait_line 'want=59904 got=59904 st=0x00000000'
press a:400 120                       # Profile 1 -> load
press a:400 180                       # Load successful -> main menu
stop
sha256sum -c "$task_state/profile-before.sha256" > "$task_state/profile-after.txt"
ffprobe -v error -show_entries stream=codec_name,sample_rate,channels -show_entries format=duration,size \
    -of default=noprint_wrappers=1 "$task_state/audio.wav" > "$task_state/audio-format.txt"
for task_expected in codec_name=pcm_s16le sample_rate=48000 channels=2 duration=45.000000 size=8640044; do
    grep -qx -- "$task_expected" "$task_state/audio-format.txt" || fail "audio format missing $task_expected"
done
ffmpeg -hide_banner -nostats -i "$task_state/audio.wav" -af astats=metadata=0:reset=0 -f null - \
    > "$task_state/audio-stats.txt" 2>&1
awk '/RMS level dB:/ { if ($NF != "-inf" && $NF < 0 && $NF > -90) good++ }
     END { exit good >= 3 ? 0 : 1 }' "$task_state/audio-stats.txt" || fail "silent or invalid mixed audio"
# Each process must reach the actual executor, not merely enumerate a device.
# Check before publishing a successful result, including on native Linux.
for task_phase in create reload; do
    python3 "$(dirname -- "${BASH_SOURCE[0]}")/check_takedawn_renderer.py" \
        "$task_state/$task_phase.log" || fail "$task_phase did not render through Vulkan"
    ! grep -aEq '\[STUB\] unresolved|\[UNIMPL\].*(stmxcsr|ldmxcsr)' "$task_state/$task_phase.log" || fail "$task_phase reached an unresolved game path"
done
printf '{"profile_bytes":59904,"metadata_bytes":54,"reload_unchanged":true,"audio_seconds":45,"sample_rate":48000,"channels":2}\n' \
    > "$task_state/result.json"
if [[ "$task_wine" == 1 ]]; then
    printf 'windows-wine\n' > "$task_state/transport.txt"
fi
echo "PASS: original profile create/reload, unchanged save hashes and non-silent stereo capture"
echo "Review private create/reload frames in $task_state; full progression and audible fidelity remain separate checks."
