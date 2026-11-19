# 2026-11-19: dedicated XFS partition for the game on prd

## Question

Can the game installation leave the shared btrfs root of the production host for a filesystem of
its own, without a reboot and without data loss, so that the VFS and Steam updates are isolated
from the other services on the host?

## Environment

- Host prd (`srv2116886`), Arch Linux, kernel 7.2.14-arch1-1, AMD EPYC 9354P, 8 vCPU, 31 GiB,
  KVM. Disk `/dev/sda` 400 GiB, GPT; before: `sda1` BIOS 1 MiB, `sda2` EFI 300 MiB, `sda3` btrfs
  root 399.7 GiB (154 GiB unallocated inside btrfs). Root mounted by filesystem UUID
  `86a96c15-9d21-45be-a21b-c81a5423eac4`.
- Game: Steam app 2799860, buildid 26449002, `nie.exe` 33,999,360 bytes, library at
  `/srv/nie/steam`.
- Tools: btrfs-progs, util-linux `sfdisk`, `resizepart`, `partx`, xfsprogs, rsync.
- All nie services on the host were stopped and uninstalled before the change.

## Method

```sh
sfdisk -d /dev/sda > /root/sda-gpt-before-nie-xfs.sfdisk          # rollback copy of the table
btrfs filesystem resize -101g /                                    # 399.70 -> 298.70 GiB, online
echo ",629145600" | sfdisk --no-reread --no-tell-kernel -N 3 /dev/sda   # sda3 = 300 GiB
echo "start=629764096, type=0FC63DAF-8483-4772-8E79-3D69D8477DE4, name=nie-game" \
  | sfdisk --no-reread --no-tell-kernel --append /dev/sda           # sda4 = rest of the disk
resizepart /dev/sda 3 629145600; partx -a -n 4 /dev/sda             # kernel view, no reboot
mkfs.xfs -L niegame -m crc=1,reflink=1,bigtime=1 /dev/sda4
mount -t xfs -o noatime,inode64,logbsize=256k,largeio /dev/sda4 /mnt/nie-new
rsync -aHAX --numeric-ids /srv/nie/steam/ /mnt/nie-new/
rsync -aHAXc --numeric-ids --dry-run --itemize-changes /srv/nie/steam/ /mnt/nie-new/
# fstab: UUID=32d2b6fb-28a2-48f5-9cad-87c5c9a42faf /srv/nie/steam xfs
#        noatime,inode64,logbsize=256k,largeio,nofail,x-systemd.device-timeout=30s 0 2
```

The new btrfs size (320,732,114,944 bytes = 626,429,912 sectors) is smaller than the new `sda3`
(629,145,600 sectors); the margin is 2,715,688 sectors (1.3 GiB).

## Result

- btrfs resize took 6.5 s; `btrfs device stats` reported no error before or after; the root
  stayed writable.
- The kernel took the new `sda3` size and `sda4` (start 629,764,096, 209,094,656 sectors,
  99.7 GiB) without a reboot.
- Copy: 1 min 21 s. Checksum pass (`rsync -c`, 54 s): no file content differs; the only
  itemised differences were ACL and xattr representations (XFS adds `trusted.SGI_ACL_FILE` beside
  `system.posix_acl_access`, same ACL) and the ACL of the new filesystem root, fixed with
  `setfacl -m u:10001:rx /srv/nie/steam`. Both trees: 63,328,936,460 bytes (`du -sb`).
- After mounting by fstab: `srv-nie-steam.mount` active, `findmnt --verify` 0 errors, buildid
  26449002 and `nie.exe` size unchanged. The btrfs copy was then deleted.
- Space after: root 299 GiB (113 GiB used), game partition 100 GiB (61 GiB used).

## Limits

- No reboot was performed to prove the fstab entry at boot (other production services run on the
  host); `nofail` keeps a missing partition from blocking boot.
- File integrity was compared copy against source, not against Steam's depot manifests; the next
  `nie steam sync` run (which skips files already up to date by manifest) is the second check.
- The root btrfs cannot grow back into `sda4` without deleting it; rollback is
  `sfdisk /dev/sda < /root/sda-gpt-before-nie-xfs.sfdisk` after removing `sda4`, then
  `btrfs filesystem resize max /`.
