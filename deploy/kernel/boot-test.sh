#!/bin/sh
# Boot test of a kernel build under QEMU/KVM with a prd-like CPU model, a virtio disk holding an XFS image and an initramfs
# that loads the XFS module, mounts the disk read-only, reads a marker file and reports the io_uring sysctl.
# Passes when the serial console shows the NIE-BOOT-OK line with the marker. Not covered here: systemd, multi-user.target
# and `nie serve /health` (they need a full root file system; that is the next stage of the pipeline).
#   usage: boot-test.sh <vmlinuz> <modules dir (…/lib/modules/<release>)> [workdir] [timeout seconds]
set -eu
kernel=${1:?path of the boot image (vmlinuz / bzImage)}
mods=${2:?modules directory of the release, for example /lib/modules/7.4.0-rc3-nie-rc}
work=${3:-$(mktemp -d)}
limit=${4:-180}
release=$(basename "$mods")
mkdir -p "$work/root/bin" "$work/root/modules" "$work/root/dev" "$work/root/proc" "$work/root/sys" "$work/root/mnt" "$work/root/lib64"

# busybox with the libraries it needs (the build host ships it dynamically linked)
bb=$(command -v busybox)
cp "$bb" "$work/root/bin/busybox"
for lib in $(ldd "$bb" | sed -n 's/.*=> \(\/[^ ]*\) .*/\1/p;s/^[[:space:]]*\(\/lib[^ ]*ld-linux[^ ]*\) .*/\1/p'); do
  mkdir -p "$work/root$(dirname "$lib")"; cp -L "$lib" "$work/root$lib"
done
for app in sh mount insmod cat echo poweroff sleep ls mkdir; do ln -sf busybox "$work/root/bin/$app"; done

# xfs and its module dependencies, in load order (modules that are built in are skipped)
order=""
add() {
  for dep in $(modinfo -b "$mods/../../.." -k "$release" -F depends "$1" 2>/dev/null | tr ',' ' '); do add "$dep"; done
  case " $order " in *" $1 "*) return ;; esac
  path=$(modinfo -b "$mods/../../.." -k "$release" -n "$1" 2>/dev/null) || return 0
  case $path in
    *.zst) zstd -dqc "$path" > "$work/root/modules/$1.ko" ;;
    *.xz) xz -dc "$path" > "$work/root/modules/$1.ko" ;;
    *.ko) cp "$path" "$work/root/modules/$1.ko" ;;
    *) return 0 ;;
  esac
  order="$order $1"
}
[ -e "$mods/modules.dep" ] || depmod -b "$mods/../../.." "$release"
add xfs

cat > "$work/root/init" <<INIT
#!/bin/sh
mount -t proc proc /proc; mount -t sysfs sys /sys; mount -t devtmpfs dev /dev
for m in$order; do insmod /modules/\$m.ko || echo "NIE-BOOT-MODULE-FAIL \$m"; done
mount -t xfs -o ro /dev/vda /mnt || echo "NIE-BOOT-MOUNT-FAIL"
echo "NIE-BOOT-OK release=\$(cat /proc/sys/kernel/osrelease) marker=\$(cat /mnt/marker) io_uring_disabled=\$(cat /proc/sys/kernel/io_uring_disabled 2>/dev/null)"
poweroff -f
INIT
chmod +x "$work/root/init"
( cd "$work/root" && find . | cpio -o -H newc --quiet | gzip -1 > "$work/initrd.gz" )

# XFS image with a marker, no mount needed (mkfs.xfs -p)
echo "nie-boot-marker-$release" > "$work/marker"
printf 'bootmarker\n0 0\nd--755 0 0\n marker ---644 0 0 %s\n$\n' "$work/marker" > "$work/proto"
truncate -s 512M "$work/xfs.img"
mkfs.xfs -q -f -L niegame -p "$work/proto" "$work/xfs.img"

run() {
  timeout "$limit" qemu-system-x86_64 -enable-kvm -cpu "$1" -smp 4 -m 2G -nographic -no-reboot \
    -kernel "$kernel" -initrd "$work/initrd.gz" -append "console=ttyS0 panic=-1 loglevel=4" \
    -drive "file=$work/xfs.img,if=virtio,format=raw" -serial "file:$work/serial.log" -monitor none >"$work/qemu.out" 2>&1 || true
}
: > "$work/serial.log"
cpu=EPYC-Genoa
run "$cpu"
if ! grep -q 'NIE-BOOT-OK' "$work/serial.log"; then cpu=host; : > "$work/serial.log"; run "$cpu"; fi
if grep -q "NIE-BOOT-OK release=$release marker=nie-boot-marker-$release" "$work/serial.log"; then
  echo "PASS cpu=$cpu $(grep NIE-BOOT-OK "$work/serial.log" | tr -d '\r')"
  echo "kernel sha256 $(sha256sum "$kernel" | cut -d' ' -f1)"
else
  echo "FAIL (serial log: $work/serial.log)"; tail -20 "$work/serial.log"; tail -5 "$work/qemu.out"; exit 1
fi
