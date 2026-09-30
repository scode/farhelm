---
kind: fixed
---

On macOS, the install script now builds and replaces `~/Applications/Farhelm.app` one install at a time, and replaces
the old bundle by moving it aside instead of deleting it in place. Two installs running at once can no longer produce a
bundle mixing two versions or one bundle nested inside another; one of them now stops and asks you to re-run it.
Interrupting the installer no longer leaves a half-deleted bundle that later installs and uninstall refuse to touch.
