---
kind: fixed
---

A session's actions menu in the sidebar could end up floating beside the wrong row. This happened when a session listed
above it ended, came back, went stale, or showed or cleared an error message while the menu was open, because that row
changed height and pushed the menu's row out from under it. The buttons still acted on the right session, but the menu
looked like it belonged to its neighbour. The menu now closes in that case, the same way it already did when the list
scrolled or reordered.
