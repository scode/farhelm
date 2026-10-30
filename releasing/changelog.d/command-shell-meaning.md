---
kind: breaking
---

Custom commands and their resume commands now run through Bash: `~`, `$VARIABLE`, `*`, `;`, `>`, `&`, `|` and backslashes
have shell meaning instead of being passed literally. Farhelm placeholders must be separate, unquoted words; quoted
placeholders are refused, and their filled values are quoted safely by Farhelm.
