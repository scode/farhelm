---
kind: fixed
---

File paths that a terminal program splits near the right edge across two lines can now be downloaded from either half.
Farhelm joins the pieces for the file lookup, including when the second line is indented. This deliberately favors
usable links over conservative recognition for that shape; unrelated lines that happen to match it can also be joined.
