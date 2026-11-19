#!/bin/sh
# QEMU arguments for cargo osdk (OSDK.toml: args = "$(sh ./qemu-args.sh)") and for
# tools/boot-linux-abi.sh. Everything is derived from the host and the environment.
#   NIE_OS_CPU   guest CPU model        (default EPYC-Genoa, the prd EPYC 9354P family)
#   NIE_OS_SMP   guest CPUs             (default 8)
#   NIE_OS_MEM   guest memory           (default 8G)
#   NIE_OS_PORT  host TCP port forwarded to guest 8790 (nie serve; default 8790)
#   NIE_OS_FS    virtiofsd socket; adds the vhost-user-fs device tagged nie-game
#   NIE_OS_DISK  raw image attached as virtio-blk
#   NIE_OS_LOG   serial log file     (default qemu-serial.log)
set -eu
mem=${NIE_OS_MEM:-8G}
args="-machine q35,kernel-irqchip=split -cpu ${NIE_OS_CPU:-EPYC-Genoa},+x2apic -smp ${NIE_OS_SMP:-8} -m $mem"
[ -w /dev/kvm ] && args="$args -accel kvm"
args="$args --no-reboot -nographic -display none -serial chardev:mux -monitor chardev:mux"
args="$args -chardev stdio,id=mux,mux=on,signal=off,logfile=${NIE_OS_LOG:-qemu-serial.log}"
args="$args -device isa-debug-exit,iobase=0xf4,iosize=0x04"
args="$args -netdev user,id=net01,hostfwd=tcp::${NIE_OS_PORT:-8790}-:8790"
args="$args -device virtio-net-pci,netdev=net01,disable-legacy=on,disable-modern=off"
args="$args -object rng-random,id=rng0,filename=/dev/urandom -device virtio-rng-pci,rng=rng0,disable-legacy=on,disable-modern=off"
if [ -n "${NIE_OS_DISK:-}" ]; then
  args="$args -drive if=none,format=raw,id=disk0,file=$NIE_OS_DISK"
  args="$args -device virtio-blk-pci,drive=disk0,disable-legacy=on,disable-modern=off"
fi
if [ -n "${NIE_OS_FS:-}" ]; then
  args="$args -object memory-backend-memfd,id=mem0,size=$mem,share=on -numa node,memdev=mem0"
  args="$args -chardev socket,id=fs0,path=$NIE_OS_FS -device vhost-user-fs-pci,chardev=fs0,tag=nie-game"
fi
# grub-rescue-iso boots through UEFI: firmware code and a private copy of the variable store.
code=$(ls /usr/share/OVMF/OVMF_CODE_4M.fd /usr/share/OVMF/OVMF_CODE.fd /usr/share/edk2/x64/OVMF_CODE.4m.fd 2>/dev/null | head -n1 || true)
vars=$(ls /usr/share/OVMF/OVMF_VARS_4M.fd /usr/share/OVMF/OVMF_VARS.fd /usr/share/edk2/x64/OVMF_VARS.4m.fd 2>/dev/null | head -n1 || true)
if [ -n "$code" ] && [ -n "$vars" ]; then
  state=${NIE_OS_STATE:-${TMPDIR:-/tmp}/nie-os-ovmf-vars-$$.fd}
  cp "$vars" "$state"
  args="$args -drive if=pflash,format=raw,unit=0,readonly=on,file=$code -drive if=pflash,format=raw,unit=1,file=$state"
fi
echo "$args"
