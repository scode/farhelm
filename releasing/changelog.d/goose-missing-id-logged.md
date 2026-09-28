---
kind: fixed
---

When Farhelm's Goose reporter is enabled but Goose does not provide its session id, the hook log now records the missing id instead of staying empty, so a Goose session that cannot be resumed can be diagnosed.
