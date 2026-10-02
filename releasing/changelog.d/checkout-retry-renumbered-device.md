---
kind: fixed
---

A new session cloned from GitHub could fail for good when its create was retried after a reboot or remount, on setups
such as btrfs subvolumes (Fedora's default `/home`), NFS or overlayfs that renumber a filesystem's device when it is
mounted. The retry refused the checkout folder Farhelm had made as "replaced", although it was untouched, and left the
folder behind. The retry now recognizes the folder and continues.
