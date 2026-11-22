---
kind: fixed
---

When Stop and suspend cannot finish cleaning up a sandbox or container session, its message no longer calls the
program an agent, so it reads correctly for shell sessions too. A sandbox whose suspension has been confirmed stays
marked Suspended during the first status check.

The detail that message includes about terminal tabs that could not be closed still comes from the supervisor, which
says "The agent stopped" for shells too; plain Stop shows the same text.
