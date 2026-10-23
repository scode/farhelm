---
kind: fixed
---

Uninstall now refuses before you confirm it when the host's own default location for Farhelm's data has a path with
control characters (such as a line break) or one that is not valid UTF-8. Before, such a path passed the confirmation and
the uninstall stopped partway, after the host's supervisor had already been disabled.

Context: in a rarer case, when another window removed the host after such a failed uninstall, the kept-data note's copy
button copied the path with its control characters; the note itself shows them escaped.
