---
kind: fixed
---

Retrying the creation of a session in a fresh GitHub checkout now refuses a malformed session id from the host, as a first attempt already did, instead of returning a session the interface cannot open.
