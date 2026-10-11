#!/usr/bin/env bash
# Prepare only the disposable WSL distribution in the native verification job.
set -euo pipefail
project=$(pwd)
test -f "$project/native-exchange-fixture/gamepkg-linux"
curl --proto '=https' --tlsv1.2 --fail --silent --show-error https://sh.rustup.rs -o /tmp/gamepkg-rustup-init.sh
sh /tmp/gamepkg-rustup-init.sh -y --profile minimal --default-toolchain 1.89.0
export PATH="$HOME/.cargo/bin:$PATH"
rustup target add wasm32-unknown-unknown
# Native wsl --exec does not load the user's interactive shell profile.
ln -sf "$HOME/.cargo/bin/rustc" /usr/local/bin/rustc
ln -sf "$HOME/.cargo/bin/cargo" /usr/local/bin/cargo
python3 "$project/tools/install_slang.py" /opt/gamepkg-slang
mkdir -p /opt/gamepkg/bin "$HOME/.config/gamepkg"
cp "$project/native-exchange-fixture/gamepkg-linux" /opt/gamepkg/bin/gamepkg
chmod 755 /opt/gamepkg/bin/gamepkg
printf 'slang_directory="/opt/gamepkg-slang"\nautomatic_update_checks=false\n' > "$HOME/.config/gamepkg/settings.toml"
/opt/gamepkg/bin/gamepkg --version
rustc --version
