---
kind: fixed
---

In the desktop app, copying text, or a terminal program setting the clipboard, could freeze terminals and the session
list for as long as the system clipboard took to accept the text, which made bursts of copies on a slow clipboard
noticeably stall the app. Clipboard writes no longer hold anything else up.
