---
kind: fixed
---

When saving a number-key shortcut in Templates failed (for example because you had been signed out), your next shortcut
change, or signing back in, could undo number keys that an agent or the `farhelm` command line had set in the meantime.
Now only the key changes you made are saved again.

An agent's or the command line's change that lands in the moment between the app reading the shortcuts and saving them
can still be overwritten.
