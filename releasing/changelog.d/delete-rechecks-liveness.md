---
kind: fixed
---

Deleting a session that the sidebar shows as stopped, without being asked to confirm, can no longer kill an agent that was in fact running again (restarted from another window or by an agent, or reported late right after a relaunch). The host now checks at the moment of deletion and refuses if the agent or any terminal tab is still running; the refusal shows as a delete error, and the row refreshes so the next Delete asks for confirmation. Replace does the same for the old session when its confirmation said nothing was running. This protection needs the host's supervisor updated too; an older supervisor deletes as before.
