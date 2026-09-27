---
kind: fixed
---

Adding a remote host that already runs Farhelm installed with the install script now finds that supervisor. The helm's check looked for `farhelm` on the PATH of a plain ssh command, which usually does not include `~/.local/bin` where the install script puts it, so the host looked empty and the helm offered to install a second copy. Installs in a custom `FARHELM_INSTALL_DIR` are still not found this way.
