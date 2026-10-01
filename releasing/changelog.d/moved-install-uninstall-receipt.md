---
kind: fixed
---

On macOS, re-running the installer after an interrupted `farhelm uninstall` no longer refuses to rebuild
`~/Applications/Farhelm.app` when the install folder has moved since (a different `FARHELM_INSTALL_DIR`, `~/.local/bin`
replaced by a link, or a renamed home folder). It used to treat the uninstall's leftover
`.Farhelm.app.uninstall-receipt` as another installation's and tell you to finish "that installation's" uninstall,
which was this one. `farhelm uninstall` itself still refuses to run after such a move until you re-run the installer from
the new folder, as before.
