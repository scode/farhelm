---
kind: fixed
---

`farhelm helm token rotate` against a running helm now waits up to 15 seconds for the new token instead of 2. Before, a rotation that was slow because the helm's database was busy could be reported as a timeout even though it completed, logging every browser out without showing the new token. If the wait still runs out, or the helm's answer is lost, the error now says the token may have been rotated and tells you to run `farhelm helm token show`.
