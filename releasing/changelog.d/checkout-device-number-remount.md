---
kind: fixed
---

A session created as a fresh GitHub checkout could refuse to restart for good after a reboot or remount on btrfs (the
default `/home` layout on Fedora), NFS, overlayfs or some device-mapper setups, because the filesystem gave the
unchanged checkout folder a new device number. Farhelm now recognizes the folder by its inode number and creation time.
On a filesystem that records no creation time, the refusal now says that a remount may be the cause.
