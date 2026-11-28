---
kind: added
---

You can now set quiet hours for ntfy alerts: during a range of hours you choose, in the local time of the machine your
helm runs on, alerts go out at a lower priority, or not at all.

Detail kept out of the entry: quiet hours are whole hours, may wrap past midnight (22:00 to 07:00 covers 22:00 through
06:59), and start off, from 22:00 to 07:00 at Low, for every existing configuration. Their priority can be any of the
five ntfy priorities or Off. An alert's priority is decided when it is sent, after any grace period. Alerts silenced by
quiet hours do not count against the five-a-minute cap. **Send test** goes out at the priority alerts would have now and
says whether quiet hours decided it; when quiet hours are set to Off it still sends, at the normal priority, so you can
check the connection, and says that real alerts are silenced.

This fragment was written with the helm side of the change, before the Settings controls existed; the PR that adds them
updates it.

The plan's settled release note (the plan `ntfy-quiet-hours-urgency`) was: "You can now set quiet hours for ntfy
alerts. Choose a local-time hour range in Settings to automatically lower or silence ntfy alert priority during
off-hours."
