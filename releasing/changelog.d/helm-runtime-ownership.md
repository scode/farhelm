---
kind: none
---

The helm keeps its state-directory lock while shutdown finishes existing work. This only changes admission of a second
helm during an exceptional shutdown; no separate release-note entry is needed.
