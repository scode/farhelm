---
kind: added
---

You can now add an Apple-silicon Mac over SSH, update Farhelm there without interrupting its sessions, and uninstall it
while keeping its data. The remote Mac must not have the Farhelm desktop app installed; after a reboot, its host menu's
Update action starts Farhelm again.

Setup, Update and uninstall also work when GNU coreutils comes first on the remote Mac's SSH command PATH.

The remote Mac runs Farhelm as a background job that restarts it after a crash, without needing tmux. The desktop app's
local supervisor is unchanged. On the SSH-only test Mac the job did not return after reboot and reconnecting; running
setup again restored it. Automatic startup at screen login is unverified.

Uninstall's check for filesystems mounted inside the installation is best effort: ambiguous mount names and unreadable
mount lists no longer block it. A nested mount with an ambiguous name can go undetected.
