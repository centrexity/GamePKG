#!/usr/bin/env bash
# Maintainer-only observation gate for the original BO3_DRIVE_LOG hook.
# This proves a bounded race and consumed inputs, not steering/audio equivalence.
set -euo pipefail
[[ $# == 1 && -f "$1" ]] || { echo "usage: $0 /private/state/drive.log" >&2; exit 2; }
awk '
/^D / {
    if (NF != 19 || $3 != "v" || $5 != "port" || $7 != "btn" ||
        $9 != "an" || $16 != "lt" || $18 != "rt") {
        print "unrecognized original drive log record" > "/dev/stderr"; bad=1; exit 2
    }
    n++
    if ($4 > v) v=$4
    if ($19 == 255) rt++
    if ($17 == 255) lt++
    if ($10 == 255 && $19 == 255) boost++
}
END {
    if (bad) exit 2
    printf "{\"race_frames\":%d,\"maximum_speed_mps\":%.4f,\"throttle_frames\":%d,\"brake_frames\":%d,\"boost_throttle_frames\":%d}\n",n,v,rt,lt,boost
    if (!(n>100 && v>30 && rt>100 && lt>0 && boost>0)) exit 1
}' "$1"
