---
kind: fixed
---

Terminal background colors now reach the right edge when a wide character wraps. If a program erases the right half of
a wide character with a different background color, that half shows the new color. Copying wrapped text keeps spaces
the program printed and skips the unused padding at the edge.
