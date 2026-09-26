---
kind: fixed
---

If a session's supervisor went away after `farhelm spawn` sent its request, spawn now says the outcome is unknown and to
check before retrying, as `farhelm agent create` already did: the new session may already be running. A supervisor's
refusal is also printed with control characters escaped, for `spawn` and `farhelm agent` alike, so a crafted message
cannot rewrite what the terminal shows. An unexpected reply now names only its kind instead of printing the whole
message.
