---
kind: fixed
---

The desktop app follows your system's Reduce Motion setting while it's open, on macOS when the window regains focus.
Attention cues flash in place, cursors stay visible, and other animations pause while the setting is on.

Caveat for curation: on Linux the app now takes the setting only from the desktop portal's reduced-motion key, as the
maintainer chose, and no longer reads GNOME's animations switch on its own. A desktop whose portal does not provide
that key gets full motion even with GNOME's animations turned off.
