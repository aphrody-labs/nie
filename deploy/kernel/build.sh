#!/bin/sh
# Build linux-nie-rc from a signed upstream tag, in the Arch container of deploy/kernel/Dockerfile, twice,
# and compare the boot images.   usage: build.sh <source|config|package|repro> <tag> [workdir]
# Runs on the build host (fr1). `nie-deploy kernel watch` decides which tag; this script does the work.
set -eu
step=${1:?step: source | config | package | repro}
tag=${2:?tag, for example v7.4-rc3}
work=${3:-$HOME/nie-kernel}
here=$(cd "$(dirname "$0")" && pwd)
dir=$work/$tag
img=nie-kernel-build
linus=$(sed -n '/Linus Torvalds/{n;s/.*"\(.*\)"/\1/p;q}' "$here/keys.toml")
jobs=${NIE_KERNEL_JOBS:-$(( $(nproc) / 2 ))}

source_tree() {
  mkdir -p "$dir" "$work/gnupg" && chmod 700 "$work/gnupg"
  export GNUPGHOME=$work/gnupg
  [ -d "$work/linux.git" ] || git init --bare -q "$work/linux.git"
  git -C "$work/linux.git" fetch -q --depth 1 --no-tags https://git.kernel.org/pub/scm/linux/kernel/git/torvalds/linux.git "refs/tags/$tag:refs/tags/$tag"
  keyserver=$(sed -n 's/^keyserver = "\(.*\)"/\1/p' "$here/keys.toml")
  curl -fsSA nie "$keyserver/${linus#????????????????????????}.asc" | gpg --quiet --import
  got=$(gpg --with-colons --fingerprint "$linus" | sed -n 's/^fpr:::::::::\(.*\):$/\1/p' | head -1)
  [ "$got" = "$linus" ] || { echo "key fingerprint $got is not the pinned $linus" >&2; exit 1; }
  signer=$(git -C "$work/linux.git" verify-tag --raw "$tag" 2>&1 | sed -n 's/^\[GNUPG:\] VALIDSIG .* \([0-9A-F]\{40\}\)$/\1/p')
  [ "$signer" = "$linus" ] || { echo "tag $tag is not signed by $linus (got '$signer')" >&2; exit 1; }
  git -C "$work/linux.git" cat-file tag "$tag" | sed -n 's/^tagger .* \([0-9]*\) [-+][0-9]*$/\1/p' > "$dir/epoch"
  ver=${tag#v}
  git -C "$work/linux.git" archive --format=tar --prefix="linux-$ver/" -o "$dir/linux.tar" "$tag"
  sha256sum "$dir/linux.tar" | cut -d' ' -f1 > "$dir/linux.tar.sha256"
  git -C "$work/linux.git" rev-parse "$tag" "$tag^{commit}" > "$dir/tag.oid"
  echo "tag $tag verified: signer $signer, tagger epoch $(cat "$dir/epoch"), tar sha256 $(cat "$dir/linux.tar.sha256")"
}

image() {
  docker build -q -t "$img" "$here" > "$work/image.id"
  docker run --rm "$img" sh -c 'pacman -Q | sort' | sha256sum | cut -d' ' -f1 > "$work/image.pkgs.sha256"
  echo "image $(cat "$work/image.id") package set $(cat "$work/image.pkgs.sha256")"
}

run_in() { # run_in <out dir> <command...>
  out=$1; shift
  mkdir -p "$out" && chmod 777 "$out"
  docker run --rm --network none --cpus "$jobs" \
    -e NIE_KERNEL_TAG="$tag" -e NIE_KERNEL_SHA256="$(cat "$dir/linux.tar.sha256")" -e SOURCE_DATE_EPOCH="$(cat "$dir/epoch")" \
    -e PKGDEST=/out -e MAKEFLAGS="-j$jobs" \
    -v "$here/PKGBUILD:/build/PKGBUILD:ro" -v "$here/prd.config:/build/prd.config:ro" -v "$here/nie-rc.fragment:/build/nie-rc.fragment:ro" \
    -v "$here/linux-nie-rc.preset:/build/linux-nie-rc.preset:ro" -v "$dir/linux.tar:/build/linux.tar:ro" -v "$out:/out" \
    "$img" "$@"
}

case $step in
  source) source_tree ;;
  config) # regenerate deploy/kernel/prd.config from the raw /proc/config.gz of prd (stdin) for this tag
    image
    cat > "$dir/raw.config"
    mkdir -p "$dir/cfg" && chmod 777 "$dir/cfg"
    docker run --rm --network none -v "$dir/linux.tar:/build/linux.tar:ro" -v "$dir/raw.config:/build/raw.config:ro" -v "$dir/cfg:/out" "$img" sh -c '
      tar -xf linux.tar && cd linux-* && cp ../raw.config .config && make olddefconfig >/dev/null && scripts/kconfig/merge_config.sh -m -O . .config /dev/null >/dev/null 2>&1; make olddefconfig >/dev/null
      cp .config /out/olddefconfig.config; scripts/diffconfig ../raw.config .config > /out/diffconfig.txt; make -s rustavailable > /out/rust.txt 2>&1 || true'
    ;;
  package) # package <tag> <workdir> <run id>
    image
    run_in "$dir/out-${4:-1}" sh -c 'cd /build && makepkg -f --nocheck --noconfirm --skippgpcheck > /out/makepkg.log 2>&1; echo "exit $?" > /out/exit'
    ;;
  repro)
    image
    run_in "$dir/out-1" sh -c 'cd /build && makepkg -f --nocheck --noconfirm --skippgpcheck > /out/makepkg.log 2>&1; echo "exit $?" > /out/exit' &
    p1=$!
    run_in "$dir/out-2" sh -c 'cd /build && makepkg -f --nocheck --noconfirm --skippgpcheck > /out/makepkg.log 2>&1; echo "exit $?" > /out/exit' &
    p2=$!
    wait $p1 $p2
    ;;
  *) echo "unknown step $step" >&2; exit 2 ;;
esac
