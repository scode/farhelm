---
kind: fixed
---

A Grok session started from a Grok install with an unfamiliar program name no longer lets a second Grok running inside
it take over which conversation **Restart** resumes; **Restart** stays unavailable for that session instead.

A wrapper that stays running above Grok and is itself named `grok`, or whose program is named `grok-` followed by
anything, now also leaves **Restart** unavailable.
