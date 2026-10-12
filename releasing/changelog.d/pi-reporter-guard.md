---
kind: fixed
---

When reading Pi's conversation details fails, Farhelm's conversation reporter inside Pi now skips that one report
quietly instead of letting the error reach Pi. Farhelm also closes its host connections cleanly when the helm shuts
down.
