---
kind: fixed
---

On macOS, a fresh GitHub checkout could not be created when the checkout root already held a folder whose name differed
from the proposed one only in letter case (for example `Bar-1` next to a proposed `bar-1`): every attempt failed and the
next preview proposed the same name again. The preview now treats such folders as taken and proposes the next free name.
