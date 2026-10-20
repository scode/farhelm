---
kind: changed
---

The helm can now finish a valid authenticated action after its client disconnects while the request is still being read.
Uploads, downloads and other operations designed to stop when their caller leaves keep that behavior.
