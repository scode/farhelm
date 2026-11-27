---
kind: changed
---

In the desktop app, hovering a file path in the terminal now says "too large to copy contents" for a file over 10 MB,
where the Shift-Cmd-click copy hint used to just disappear.

Context for curation: files up to 10 MB keep the copy hint as before, and a file whose size the host did not report
shows neither hint. On Linux the same applies with Shift-Ctrl-click, except that an image shows neither hint at any
size, since images cannot be copied there yet. The hint is plain text in the same style as the card's other hints. The
plan asked for no release note highlight.
