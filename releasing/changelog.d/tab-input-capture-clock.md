---
kind: fixed
---

Typing in one of a session's extra terminal tabs no longer affects which conversation Farhelm links to that session. Before, the first keystroke in any tab counted as the start of the agent's conversation, so if you used a tab before prompting the agent, restart could fail to offer resuming its conversation, or could pick up a conversation you ran in the tab instead. This applies to agents whose conversation Farhelm finds by scanning; agents that report their conversation through hooks were not affected.
