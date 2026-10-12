#!/bin/sh
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
os=$(uname -s)
case "$os" in
  Linux)
    src="$root/target/release/libobs_gt_title.so"
    dest="$root/dist/obs-gt-title-linux"
    name=libobs_gt_title.so
    ;;
  Darwin)
    src="$root/target/release/libobs_gt_title.dylib"
    dest="$root/dist/obs-gt-title-macos"
    name=libobs_gt_title.dylib
    ;;
  *)
    echo "On Windows, run scripts/package.ps1." >&2
    exit 1
    ;;
esac
if [ ! -f "$src" ]; then
  echo "Release build not found. Run: cargo build -p obs-gt-title --release" >&2
  exit 1
fi
mkdir -p "$dest"
cp "$src" "$dest/$name"
echo "Packaged $dest"
