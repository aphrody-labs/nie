#!/bin/sh
# Boots the Linux-ABI Asterinas kernel of vendor/asterinas under QEMU/KVM with nie in the initramfs
# and the game directory shared read-only through virtiofs (tag nie-game, mounted on /game).
# Usage: boot-linux-abi.sh <game-dir> <nie-static-binary>
#   ASTERINAS  checkout holding the kernel (default: <repo>/vendor/asterinas)
#   OSDK_BIN   directory with the cargo-osdk built from that checkout
#   NIE_OS_*   QEMU settings, see os/qemu-args.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
game=$(cd "${1:?game directory}" && pwd)
nie=$(realpath "${2:?static nie binary}")
asterinas=$(realpath "${ASTERINAS:-$here/../../vendor/asterinas}")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
sh "$here/mkinitramfs.sh" "$nie" "$work/initramfs.cpio.gz"
export NIE_OS_FS="$work/virtiofsd.sock"
args=$(sh "$here/../qemu-args.sh")
cd "$asterinas/kernel"
[ -n "${OSDK_BIN:-}" ] && PATH="$OSDK_BIN:$PATH"
exec cargo osdk run --target-arch x86_64 --release \
  --initramfs "$work/initramfs.cpio.gz" \
  --config "qemu.args=$args" \
  --qemu-with-daemon "virtiofsd --socket-path=$NIE_OS_FS --shared-dir=$game --readonly --sandbox none --cache auto"
