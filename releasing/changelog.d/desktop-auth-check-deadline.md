---
kind: fixed
---

The desktop app no longer waits forever on "Starting Farhelm…" when the embedded helm accepts the saved-credential check but never answers it. The check now gives up after 5 seconds and shows an error.
