---
kind: fixed
---

Sessions now start when the path to Farhelm or its state directory contains characters a shell treats specially, such
as braces or `!`. Before, the login shell sessions start under could expand those characters and launch the wrong
path.
