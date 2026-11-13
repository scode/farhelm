---
kind: breaking
---

Farhelm no longer has a browser interface. `farhelm helm run` serves no page and offers no browser sign-in, and the
`farhelm helm token show` and `farhelm helm token rotate` commands are removed. A Linux helm installed with
`farhelm helm setup` keeps running, but nothing can connect a GUI to it.

The GUI is being rebuilt as a native desktop app for macOS and Linux, which replaces both the browser interface and the
current Mac app. This draft is written while that migration is under way: builds from the development branch in this
period may have no working GUI at all, and the entry is revised as the new app lands. Agents and the command line keep
working as before.
