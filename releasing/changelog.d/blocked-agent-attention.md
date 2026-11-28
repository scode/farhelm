---
kind: changed
---

When an agent says it is blocked (`farhelm agent notify`), the alert no longer goes into the navigation rail's
notifications bell. Instead the session counts as waiting for you: it sorts with active work under most
recent activity, even while muted, and shows under the **Waiting** filter, until you switch to it, type into it, or it
stops, restarts, ends or is deleted. Blocked-agent notices already in the bell are removed when you upgrade.

Context for curation: this fragment covers the first PR of a three-PR stack (the plan
`blocked-notify-attention-and-debounce`), and the next two PRs revise it. The second makes the blocked session show the
red waiting dot and play the waiting sound in the desktop app, and moves "Ask agents to notify when blocked" into its
own Settings section so it works without ntfy. The third adds a configurable grace period (10 seconds by default) before
a blocked or waiting ntfy alert goes out, so switching to or interacting with that session in time sends none. The
plan's settled release note is: "Blocked-agent alerts now use the red waiting indicator and sound instead of the
navigation bell. A configurable grace period in Settings also gives you a few seconds to switch to or interact with the
session before an ntfy alert is sent." After this first PR alone, the session list's order and Waiting filter already
treat a blocked session as waiting, but its row does not yet look different.
