#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
os=$(uname -s)
case "$os" in
  Linux)
    src="$root/target/release/libobs_gt_title.so"
    dest="${XDG_CONFIG_HOME:-$HOME/.config}/obs-studio/plugins/obs-gt-title/bin/64bit"
    name=libobs_gt_title.so
    ;;
  Darwin)
    src="$root/target/release/libobs_gt_title.dylib"
    dest="$HOME/Library/Application Support/obs-studio/plugins/obs-gt-title/bin/64bit"
    name=libobs_gt_title.dylib
    ;;
  *)
    echo "On Windows, run scripts/install.ps1." >&2
    exit 1
    ;;
esac
if [ ! -f "$src" ]; then
  echo "Release build not found. Run: cargo build -p obs-gt-title --release" >&2
  exit 1
fi
mkdir -p "$dest"
cp "$src" "$dest/$name"
echo "Installed $name to $dest"
