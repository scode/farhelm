---
kind: fixed
---

When Restart with turns a custom agent's conversation reporting on or off, a conversation the restarted agent reports
as it starts is now accepted or refused under the new setting, not the old one. The restart's reply also shows whether
Restart can resume the conversation as the new settings allow.

Before, Restart with saved the edited launch only after the new agent was running, so a report sent right at startup
could be refused although reporting had just been turned on (losing the conversation the user started), or accepted
although it had just been turned off. A Restart with that fails before anything starts keeps the session's earlier
settings; one that fails once its agent has started, or may have, keeps the new ones.
