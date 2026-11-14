---
kind: breaking
---

Farhelm no longer has a browser interface. `farhelm helm run` serves no page and offers no browser sign-in, and the
`farhelm helm token show` and `farhelm helm token rotate` commands are removed. A Linux helm installed with
`farhelm helm setup` keeps running, but nothing can connect a GUI to it.

The GUI is being rebuilt as a native desktop app for macOS and Linux, which replaces both the browser interface and the
old Mac app. The old Mac app's window is already gone: until the new app lands, the desktop app starts its helm and
local supervisor and opens an empty window. This draft is written while that migration is under way, so builds from the
development branch in this period have no working GUI, and the entry is revised as the new app lands. Agents and the
command line keep working as before.
