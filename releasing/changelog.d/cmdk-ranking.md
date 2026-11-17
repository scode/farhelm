---
kind: changed
---

The quick switcher (Cmd+K, or Ctrl+Shift+K on Linux) now lists sessions whose title holds what you typed above those
whose title only has the letters scattered through it, even when the scattered match was active more recently. Matches
at the start of a word come first, and the highlighted letters show the match that put each row where it is.

For the curator: title matches still come before matches only in the host name or folder, and the same ordering applies
within that second group. Which sessions match at all is unchanged, and so are the `tl:` and Cmd+J template searches.
