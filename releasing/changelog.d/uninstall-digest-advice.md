---
kind: fixed
---

If an install or update was interrupted at the very end, `farhelm uninstall` could refuse with a bare "does not match its
recorded SHA-256 digest", which read like tampering. It now says to re-run the installer, which records the files it
installed, and then run uninstall again.
