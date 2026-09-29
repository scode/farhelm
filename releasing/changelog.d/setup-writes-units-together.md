---
kind: fixed
---

`farhelm helm setup` could fail after rewriting the supervisor's service file but not the helm's, for example when it
could not ask systemd whether the helm was running. If the run changed the state directory or the binary path, the two
services could then start looking in different places after the next reload or reboot. Setup now checks both services
before writing either file, and puts the first file back if the second cannot be written.
