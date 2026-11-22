---
kind: fixed
---

When **Rebuild** fails for a container recipe, Farhelm keeps the previous images on that host so new sessions can
still use them while they are fresh. A launch already waiting for the Rebuild reports its error and can be retried.
