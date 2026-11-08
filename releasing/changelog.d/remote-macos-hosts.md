---
kind: added
---

You can now add an Apple-silicon Mac over SSH, update Farhelm there without interrupting its sessions, and uninstall it
while keeping its data. The remote Mac must not have the Farhelm desktop app installed; after a reboot, its host menu's
Update action starts Farhelm again.

The remote Mac runs Farhelm as a background job that restarts it after a crash, without needing tmux. The desktop app's
local supervisor is unchanged. On the SSH-only test Mac the job did not return after reboot and reconnecting; running
setup again restored it. Automatic startup at screen login is unverified.
