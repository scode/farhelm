---
kind: added
---

When dragging over text in a program that handles the mouse itself (Codex, or vim with mouse support on) copies
nothing, the terminal now says why, once per page: the program keeps the selection to itself, so use its own copy
command, or hold Option (Shift outside macOS) while dragging to select and copy in Farhelm instead. In a Codex session
the notice gives Codex's own way to copy what you highlighted in its prompt box: press Ctrl+C while the text is still
highlighted. Without a highlight, Ctrl+C clears your draft in Codex, so the notice says so.
