---
kind: fixed
---

The restart confirmation in a session's header no longer says the agent is "still running" when its status is unknown, as it is for a moment after a session is created or restarted. It now says the agent may still be running and will be stopped first if so, and follows the status if it changes while the confirmation is open.
