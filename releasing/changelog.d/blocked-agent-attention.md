---
kind: changed
---

When an agent says it is blocked (`farhelm agent notify`), its session now shows as waiting, with the red dot and the
waiting sound, instead of a notice in the notifications bell. **Ask agents to notify when blocked** now has its own card
in Settings → Notifications and works without ntfy alerts.

Context for curation: this fragment covers the first two PRs of a three-PR stack (the plan
`blocked-notify-attention-and-debounce`); the third revises it again, adding a configurable grace period (10 seconds by
default) before a blocked or waiting ntfy alert goes out, so switching to or interacting with that session in time sends
none. The plan's settled release note is: "Blocked-agent alerts now use the red waiting indicator and sound instead of
the navigation bell. A configurable grace period in Settings also gives you a few seconds to switch to or interact with
the session before an ntfy alert is sent."

Detail kept out of the entry: a blocked session sorts with active work and shows under the Waiting filter, even while
muted, until the user switches to it, types into it, or it stops, restarts, ends or is deleted. Blocked-agent notices
already in the bell are removed on upgrade. Saved settings keep doing what they did: a configuration with ntfy alerts
and their **Agent is blocked** switch both on keeps asking agents to notify; any other keeps not asking, until the user
changes the new switch.
