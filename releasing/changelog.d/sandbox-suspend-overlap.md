---
kind: changed
---

Suspending or resuming a Tensorlake sandbox no longer blocks launching sessions on your other hosts or changing other
hosts while it runs. Suspend and Resume of a sandbox are now unavailable only while a session launch in the same window
is starting on, or waking, that same sandbox.

This covers Suspend, Resume and Stop and suspend from a sandbox session's row menu or the top of an open session, and
Suspend and Resume in Hosts. Before, any of them kept every launch and host change in that window waiting until it
finished, which for a Resume could take up to a couple of minutes.
