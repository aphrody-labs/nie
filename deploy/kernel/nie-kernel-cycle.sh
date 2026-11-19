#!/bin/sh
# One pipeline run on the build host (fr1): watch -> signed tag -> two package builds -> same boot image hash -> QEMU boot test -> record.
# Exits 0 when there is nothing to do or the run recorded `ok`; the new package is left in <work>/<tag>/out-1 for the prd rollout
# (`pacman -U`, then `grub-mkconfig`, then `grub-reboot`: see docs/research/log/2026-11-19-kernel-rc-pipeline.md).
#   env: NIE_KERNEL_WORK (default $HOME/nie-kernel), NIE_KERNEL_STATE (default <work>/state), NIE_KERNEL_BIN (default nie-kernel)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
work=${NIE_KERNEL_WORK:-$HOME/nie-kernel}
state=${NIE_KERNEL_STATE:-$work/state}
bin=${NIE_KERNEL_BIN:-nie-kernel}

rc=0
decision=$("$bin" --state "$state" watch) || rc=$?
echo "$decision"
case $rc in
  0) ;;
  10 | 11) exit 0 ;;
  *) exit "$rc" ;;
esac
tag=$(echo "$decision" | jq -r '.tag')
dir=$work/$tag

fail() { "$bin" --state "$state" record --tag "$tag" --status failed; echo "pipeline failed: $*" >&2; exit 1; }

"$here/build.sh" source "$tag" "$work" || fail "tag verification"
for run in 1 2; do
  "$here/build.sh" package "$tag" "$work" "$run" || fail "package build $run"
  [ "$(cat "$dir/out-$run/exit" 2>/dev/null)" = "exit 0" ] || fail "makepkg $run: $(tail -3 "$dir/out-$run/makepkg.log" 2>/dev/null)"
done
image_sha() { tar --zstd -xOf "$(ls "$dir/out-$1"/linux-nie-rc-[0-9]*.pkg.tar.zst | head -1)" --wildcards 'usr/lib/modules/*/vmlinuz' | sha256sum | cut -d' ' -f1; }
sha1=$(image_sha 1); sha2=$(image_sha 2)
[ "$sha1" = "$sha2" ] || fail "boot image differs between builds: $sha1 $sha2"

pkg=$(ls "$dir/out-1"/linux-nie-rc-[0-9]*.pkg.tar.zst | head -1)
rm -rf "$dir/extract" && mkdir -p "$dir/extract" && tar --zstd -xf "$pkg" -C "$dir/extract"
release=$(ls "$dir/extract/usr/lib/modules")
"$here/boot-test.sh" "$dir/extract/usr/lib/modules/$release/vmlinuz" "$dir/extract/usr/lib/modules/$release" "$dir/boot-test" 180 || fail "boot test"

"$bin" --state "$state" record --tag "$tag" --status ok --bzimage-sha256 "$sha1"
echo "ready: $pkg (boot image sha256 $sha1)"
