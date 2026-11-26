---
kind: fixed
---

On Linux, pasting into a text field, such as an SSH sign-in prompt, no longer freezes the window while the app you copied from is slow to hand over what you copied.

Before, the window could hang for up to four seconds per paste under Wayland. The pasted text now arrives a moment later instead, and if you type in the field, move its cursor, leave it or paste again before it arrives, that paste is dropped. Pasting into a session's terminal is unchanged.
