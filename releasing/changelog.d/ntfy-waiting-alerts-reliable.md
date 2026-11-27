---
kind: changed
---

ntfy alerts for an agent waiting on you now arrive about 3 seconds after it starts waiting, instead of 10.

Settings → Notifications now has an **Enable ntfy alerts** master switch and an **Agent is waiting for an answer** switch
under it, which says which agents' questions Farhelm can see. A line beside **Send test** says when alerts are saved off,
or when the form has unsaved changes, since a test alert arriving does not mean real alerts are on. The waiting switch is
on for settings saved before it existed, so upgrading changes nothing for people who already had alerts on.

As before, a session that is already waiting when alerts are turned on alerts only for its next wait. Internally the helm
no longer tracks waiting sessions while alerts are off; that has no visible effect beyond the above.

The plan's release note (which also covers the blocked-agent alerts still to come in the same plan) is the entry the
release curation should start from; this fragment covers the waiting-alert part on its own.
