#!/bin/sh
# Promote the running kernel to the GRUB default once the host is healthy.
# Flow: `grub-reboot <entry>` boots a new kernel once; this runs at boot, and only when the running kernel is not the saved
# default. Healthy (required units active, required mounts present) -> `grub-set-default <running entry>`.
# Not healthy within the timeout -> the saved default stays and the host reboots into it. The one-shot `next_entry` is already
# cleared by GRUB, so the next boot is the saved fallback and there is no loop.
# Config: /etc/kernel-promote/required  (one `unit <name>` or `mount <path>` per line), NIE_PROMOTE_TIMEOUT seconds (default 300).
set -eu
conf=${NIE_PROMOTE_CONF:-/etc/kernel-promote/required}
timeout=${NIE_PROMOTE_TIMEOUT:-300}
log() { logger -t nie-kernel-promote -p daemon.notice -- "$*"; echo "$*"; }

image=$(sed -n 's/.*BOOT_IMAGE=\([^ ]*\).*/\1/p' /proc/cmdline)
root=$(sed -n 's/.*root=UUID=\([^ ]*\).*/\1/p' /proc/cmdline)
name=$(basename "$image"); name=${name#vmlinuz-}
[ -n "$name" ] && [ -n "$root" ] || { log "cannot derive the running entry from /proc/cmdline"; exit 0; }
running=gnulinux-$name-advanced-$root
saved=$(grub-editenv list | sed -n 's/^saved_entry=//p')
if [ "$running" = "$saved" ]; then log "running $running is the saved default"; exit 0; fi

missing() {
  while read -r kind arg; do
    case $kind in
      unit) systemctl is-active --quiet "$arg" || echo "unit $arg" ;;
      mount) findmnt --mountpoint "$arg" >/dev/null || echo "mount $arg" ;;
    esac
  done < "$conf"
}

end=$(( $(date +%s) + timeout ))
while :; do
  bad=$(missing)
  if [ -z "$bad" ]; then
    grub-set-default "$running"
    log "healthy on $(uname -r): saved default $saved -> $running"
    exit 0
  fi
  [ "$(date +%s)" -lt "$end" ] || break
  sleep 5
done
log "unhealthy on $(uname -r) after ${timeout}s ($(echo $bad)): rebooting into saved default $saved"
systemctl --no-block reboot
