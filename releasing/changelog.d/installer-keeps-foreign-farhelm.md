---
kind: fixed
---

The installer no longer deletes an existing file named `farhelm` (or `farhelm-desktop` on macOS) that it did not put there itself, such as a wrapper script of your own in a custom `FARHELM_INSTALL_DIR`. Such a file is now kept next to it as `farhelm.replaced-<time>`, and the installer's closing message says so. A Farhelm installed before the installer started recording its own files is treated the same way on its next update, which leaves one old copy behind that you can delete.
