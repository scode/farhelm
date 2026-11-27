---
kind: fixed
---

Restarting a Claude Code session before you sent its first prompt no longer fails with "No conversation found" and an
empty terminal: Claude had saved nothing to resume, so Restart now starts Claude afresh in the same directory.

Restart decides this by checking, once the old Claude has stopped, whether the conversation file Claude reported
exists. Beyond the plan's release note: a conversation whose file was removed later (by Claude's own cleanup, say) also
starts afresh instead of failing, Restart with starts the replacement's command, and a wrapper that gives Claude a
different view of its home directory makes every Restart start afresh (documented on the Claude page). Sessions
captured by an older build resume exactly as before, unchecked.
