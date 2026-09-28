---
kind: fixed
---

On a Mac with two Farhelm installations in different directories, `farhelm uninstall` for the one that does not own `~/Applications/Farhelm.app` now says which installation owns the app and to uninstall that one first, or move the app aside. It used to suggest rerunning the installer, which would have rebuilt the app from the wrong installation and taken it away from the other one.
