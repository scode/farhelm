---
kind: fixed
---

`farhelm agent sessions`, `farhelm agent hosts`, and the other agent command tables now show invisible characters in
titles, paths, and error messages (zero-width spaces, soft hyphens, byte-order marks, and similar) as visible escapes
such as `\u{200b}`, as the browser already did. Before, two different session titles could print identically.
