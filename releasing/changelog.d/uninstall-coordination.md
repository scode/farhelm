---
kind: fixed
---

`farhelm uninstall` now stops with nothing removed, and says why, if an install or update of the same installation is
running, if `farhelm helm setup` is changing its services, if what it was about to remove changed after you confirmed, or on macOS if the Farhelm
desktop app (or another Farhelm helm or supervisor using its data folder) is running. Before, it could delete files an
update had just installed, or remove a service setup was rewriting. An install or update started while uninstall runs is
refused instead. Stopping your sessions before uninstalling is still up to you.
