---
kind: fixed
---

With Farhelm's conversation hooks turned off for an agent (`FARHELM_AGENT_HOOKS`), choosing a different conversation
with **Restart with** now forgets the previous one, so a later **Restart** no longer reopens the conversation you left.

Without hooks Farhelm never learns which conversation the agent is in, so after Restart with, plain Restart is
unavailable; Restart with can still enter a conversation you name. Grok is unaffected, since its conversation reports
come from callbacks you configure yourself.
