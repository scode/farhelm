---
kind: fixed
---

On macOS, if `farhelm uninstall` was interrupted at the very end and you then reinstalled a different version, every
later uninstall refused with advice to re-run the installer, which did not help. The installer now removes the leftover
file the interrupted uninstall left next to `Farhelm.app`, when it belongs to that installation.
