---
kind: fixed
---

The helm now starts on a Linux machine with no CA certificates installed. Adding or updating a host still needs them: install the `ca-certificates` package and restart the helm.
