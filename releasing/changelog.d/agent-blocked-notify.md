---
kind: added
---

An agent that is blocked can now tell you so with `farhelm agent notify "<what is blocked: why>"`. The report shows in
the Notifications bell, naming the session and host, and, with **Agent is blocked** turned on under ntfy alerts in
Settings → Notifications, also reaches your phone as an ntfy alert.

Each session keeps one such notice; a new report replaces it as unread, and reading it clears it. The command needs no
approval and works from any kind of host, since it goes through the helm like other `farhelm agent` commands.

With **Agent is blocked** on, Farhelm also asks Claude Code, Codex, Muse Code, Goose, Pi and OMP to run it before they
stop because they are blocked, in sessions started or restarted after you save. Settings → Notifications lets you
describe what counts as blocked in your own words, and shows Farhelm's own description while you have not.

The plan's release note covers the whole feature and is where the release curation should start.
