---
kind: changed
---

Custom agents: a conversation ID that starts with `claude:` is now refused, like the other prefixes Farhelm reserves
for its own use (`pi:`, `omp:`, `codex:` and `grok:`).

A custom agent session whose already-stored ID starts with `claude:` also stops offering Restart, as happened when the
other prefixes were reserved; such an ID is very unlikely.

Farhelm now stores a Claude session's conversation together with the transcript file Claude reported, under that
prefix, so that Restart can tell whether Claude saved the conversation yet. That Restart change comes in a later PR of
the same plan, with its own entry; this PR changes nothing a Claude user sees.
