#!/usr/bin/env bash
set -euo pipefail
project_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
export GAMEPKG_HOME="$project_dir/.demo-library"
cli="$project_dir/target/release/gamepkg"
gui="$project_dir/target/release/gamepkg-gui"
if [[ ! -x "$cli" || ! -x "$gui" ]]; then
    echo 'Build first: cargo build --locked --release --all-features --bins' >&2
    exit 1
fi
if [[ ! -f "$GAMEPKG_HOME/com.gamepkg.collect-stars.gamepkg" ]]; then
    demo_stage="$(mktemp -d)"
    trap 'rm -rf -- "$demo_stage"' EXIT
    python3 -c 'import os,sys; open(sys.argv[1], "w").write(os.urandom(32).hex())' "$demo_stage/creator.key"
    "$cli" pack "$project_dir/examples/good/collect-stars" -o "$demo_stage/stars.gamepkg" --key "$demo_stage/creator.key"
    "$cli" install "$demo_stage/stars.gamepkg" --trust-creator
    rm -rf -- "$demo_stage"
    trap - EXIT
fi
"$cli" build com.gamepkg.collect-stars
exec "$gui" --library "$GAMEPKG_HOME" --play com.gamepkg.collect-stars
