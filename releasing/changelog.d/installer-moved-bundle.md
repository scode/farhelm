---
kind: fixed
---

On macOS, installing after the install directory moved (a different `FARHELM_INSTALL_DIR`, `~/.local/bin` replaced by a
symlink, or a renamed home directory) no longer refuses `~/Applications/Farhelm.app` and leaves it on the old version:
the installer now recognizes the bundle as its own and rebuilds it. A bundle that belongs to another installation that
is still in place is still refused, and the message now says which directory that installation is in.
