---
kind: fixed
---

The installer no longer reads `FARHELM_RELEASE_BASE_URL`, the helm's release mirror setting. If that variable was set in your shell, running the installer downloaded Farhelm from the mirror, possibly over plain HTTP and without the signature check the helm applies, instead of from GitHub over HTTPS. The installer now always downloads from GitHub over HTTPS.
