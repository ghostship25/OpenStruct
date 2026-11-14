#!/usr/bin/env bash
# Build OpenStruct for Linux and collect the release binaries.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

cargo build --release --locked -p openstruct -p openstruct-gui

dest="$root/builds/linux"
mkdir -p "$dest"
cp target/release/openstruct-gui "$dest/OpenStruct"
cp target/release/openstruct "$dest/openstruct-cli"
cp target/release/libopenstruct.so "$dest/libopenstruct.so"
(cd "$dest" && sha256sum OpenStruct openstruct-cli libopenstruct.so > SHA256SUMS.txt)

echo "Artifacts written to $dest"
