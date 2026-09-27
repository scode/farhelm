---
kind: fixed
---

If the connection to a host drops at the start of the install step while setting up or updating Farhelm there, the large uploaded temporary file (`.farhelm.farhelm-tmp-…`) is now removed instead of being left in the host's farhelm or bin directory.
