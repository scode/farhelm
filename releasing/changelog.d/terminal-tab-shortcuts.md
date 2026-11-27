---
kind: added
---

Cmd+T and Cmd+W now open and close terminal tabs, as in a typical terminal app: Cmd+T opens a new shell tab, and Cmd+W
on a shell tab closes that tab, asking first unless you skip session confirmations. Cmd+W on the session's main terminal
still deletes the session. On Linux the chords are Ctrl+Shift+T and Ctrl+Shift+W, and Ctrl+Shift+Backspace still
deletes the session. Deleting a session that still has terminal tabs open now always asks first, even with **Skip
session confirmations** on, whether you delete it from its row, its Delete button or the Delete shortcut; deleting
several marked sessions at once still follows the switch.

Context for curation: the plan's release note for this work reads "Cmd+T and Cmd+W now open and close individual
terminal tabs like in a typical terminal emulator. Pressing Cmd+W on the main agent tab still deletes the session, and
deleting a session with open terminal tabs now always asks for confirmation." What was built matches it; the draft
above adds the Linux chords and the bulk Delete caveat. The Settings switch's own help still says it deletes on the
first click; it does not mention the open-tabs exception (nor the sandbox and container Delete it never skipped).
