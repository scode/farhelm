---
kind: changed
---

When Restart is greyed out because Farhelm has not captured the session's conversation yet, its hover text now says,
for that agent, when Restart normally becomes available: for Codex, once you submit your first prompt; for Pi and OMP,
after the agent's first reply; for Claude, a few seconds after it starts. Once the session has ended it says the
conversation was never captured instead. Either way it asks you to send feedback if that is not what happened. The
refusal an agent gets from `farhelm agent restart` says the same.
