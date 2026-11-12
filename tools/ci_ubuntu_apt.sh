#!/usr/bin/env bash
# Maintainer CI dependency installation. All CI runners using this helper are
# Ubuntu 24.04; their build dependencies come from Ubuntu, not preinstalled
# third-party feeds. Keep normal archive signature checks enabled.
set -euo pipefail
[[ $# -gt 0 ]] || { echo 'Pass apt-get arguments.' >&2; exit 2; }
ci_apt_sources=$(mktemp -d)
trap 'rm -rf -- "$ci_apt_sources"' EXIT
cat > "$ci_apt_sources/ubuntu.list" <<'SOURCES'
deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://archive.ubuntu.com/ubuntu noble main restricted universe multiverse
deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://archive.ubuntu.com/ubuntu noble-updates main restricted universe multiverse
deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://archive.ubuntu.com/ubuntu noble-backports main restricted universe multiverse
deb [signed-by=/usr/share/keyrings/ubuntu-archive-keyring.gpg] https://security.ubuntu.com/ubuntu noble-security main restricted universe multiverse
SOURCES
ci_apt_command=(apt-get)
if [[ $EUID != 0 ]]; then ci_apt_command=(sudo apt-get); fi
"${ci_apt_command[@]}" -o "Dir::Etc::sourcelist=$ci_apt_sources/ubuntu.list" \
    -o Dir::Etc::sourceparts=- -o Acquire::Retries=3 "$@"
