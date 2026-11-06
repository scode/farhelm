---
kind: fixed
---

The web UI and desktop app now include the license texts for the code Farhelm's own terminal took from the xterm.js
terminal library and from Visual Studio Code, such as how keys, clicks and the mouse wheel are turned into terminal
input.

This is separate from the xterm.js license texts added for the bundled xterm.js itself: those go away together with
xterm.js once old terminals are dropped, while this code stays. Release builds of `farhelm` carry the same text, since
they embed the web UI.
