---
kind: fixed
---

Dropping a file on the web page anywhere but a connected terminal no longer replaces Farhelm with that file. A terminal
that is still loading or reconnecting is hidden, so a file dropped on it, or beside it, fell through to the browser,
which opened the file in the tab and took every terminal and upload with it. Such a drop on a terminal pane now shows the
same "not connected, nothing was attached" message a disconnected terminal gives, and a drop elsewhere on the page does
nothing. Dropping text into a text field works as before.
