---
kind: added
---

Hovering a link a program printed in the terminal now warns loudly when the link's text is a web address that does not
match where the link actually goes: the usual small display turns into a large red warning that shows the text next to
the real destination. Text that is not a web address (a file name, "click here") keeps the usual display, and a
difference of only a trailing slash does not count. A long web address that wraps onto a second line can show the
warning even when it is honest, because only the hovered line's part of the text is compared.
