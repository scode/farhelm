---
kind: added
---

An agent that is blocked can now tell you so with `farhelm agent notify "<what is blocked: why>"`. The report shows in
the Notifications bell, naming the session and host, and, with **Agent is blocked** turned on under ntfy alerts in
Settings → Notifications, also reaches your phone as an ntfy alert.

Each session keeps one such notice; a new report replaces it as unread, and reading it clears it. The command needs no
approval and works from any kind of host, since it goes through the helm like other `farhelm agent` commands.

A later PR in the same plan makes Farhelm ask supported agents to run this command when they are blocked; until then an
agent only does so if you tell it to. The plan's release note covers the whole feature and is where the release curation
should start.
