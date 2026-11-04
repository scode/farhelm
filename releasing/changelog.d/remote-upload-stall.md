---
kind: fixed
---

Adding or updating a remote host no longer hangs when copying Farhelm to the host stalls. It now fails with "the payload
transfer stalled", so the host is free for another attempt.

The hang could happen when Farhelm's ssh connection to the host was shared between commands, which is Farhelm's normal
setup: the shared connection kept the copy's output open while Farhelm still held its input. This was found by reading
the code during review of the uninstall fix, not reported from a real stall.
