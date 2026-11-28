---
kind: changed
---

When an agent says it is blocked (`farhelm agent notify`), its session now shows as waiting, with the red dot and the
waiting sound, instead of a notice in the notifications bell. A new **Grace period** for ntfy alerts (10 seconds by
default) holds back a waiting or blocked alert, and sends none if you switch to, type into or click in that session
first.

Detail kept out of the entry: the grace period is in Settings → Notifications → ntfy alerts, with Off, 5 s, 10 s, 30 s,
1 min and 2 min (up to 10 minutes through the helm's route); Off sends alerts as before. Pressing a key or scrolling
inside the open session while it shows as waiting counts too, but only in Farhelm's active window: window focus alone,
or merely having the session open, does not withdraw an alert. An alert is also withdrawn if what it reports ends before
the grace period does: a waiting alert when the session stops waiting or its host disconnects, a blocked alert when the
mark clears, either when the host is removed. A blocked alert survives a disconnect. With the default, a
waiting alert now arrives about 13 seconds after the wait begins rather than 3.

Also user-visible, left out of the entry only to keep it short: **Ask agents to notify when blocked** now has its own
card in Settings → Notifications and works without ntfy alerts.
A blocked session sorts with active work and shows under the Waiting filter, even while muted, until the user switches
to it, types into it, or it stops, restarts, ends or is deleted. Blocked-agent notices already in the bell are removed
on upgrade. Saved settings keep doing what they did: a configuration with ntfy alerts and their **Agent is blocked**
switch both on keeps asking agents to notify; any other keeps not asking, until the user changes the new switch.

The plan's settled release note (the plan `blocked-notify-attention-and-debounce`) was: "Blocked-agent alerts now use
the red waiting indicator and sound instead of the navigation bell. A configurable grace period in Settings also gives
you a few seconds to switch to or interact with the session before an ntfy alert is sent."
