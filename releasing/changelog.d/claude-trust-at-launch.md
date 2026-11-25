---
kind: fixed
---

Farhelm applies Claude Code's workspace trust only when it starts Claude Code. Starting another agent in a new managed
checkout leaves your Claude settings alone.

On a host whose supervisor has not updated yet, a Claude Code session started in a new managed checkout is still
trusted, but archiving that checkout does not remove the trust entry until the host updates. Custom command and older
`claude` command launches in a new checkout no longer get the trust edit or the trust hint.
