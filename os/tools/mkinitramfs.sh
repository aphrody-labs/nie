#!/bin/sh
# Packs a reproducible initramfs: static busybox, the static musl nie and os/initramfs/init.
# Usage: mkinitramfs.sh <nie-static-binary> <output.cpio.gz> [busybox]
set -eu
nie=${1:?nie static binary}
out=${2:?output .cpio.gz}
busybox=${3:-$(command -v busybox-static || command -v busybox)}
here=$(cd "$(dirname "$0")/.." && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/bin" "$stage/proc" "$stage/sys" "$stage/dev" "$stage/tmp" "$stage/game"
install -m 0755 "$busybox" "$stage/bin/busybox"
install -m 0755 "$nie" "$stage/bin/nie"
install -m 0755 "$here/initramfs/init" "$stage/init"
for applet in sh mount grep ls tr mkdir poweroff cat ip ifconfig ps nc sha256sum; do
  ln -s busybox "$stage/bin/$applet"
done
find "$stage" -exec touch -h -d @0 {} +
(cd "$stage" && find . -mindepth 1 | LC_ALL=C sort | cpio -o -H newc --reproducible --owner=0:0 --quiet) | gzip -9n > "$out"
sha256sum "$out"
