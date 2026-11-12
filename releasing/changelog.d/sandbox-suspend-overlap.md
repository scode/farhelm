---
kind: changed
---

Suspending or resuming a Tensorlake sandbox no longer blocks launching sessions on your other hosts or changing other
hosts while it runs. Suspend and Resume of a sandbox are now unavailable only while a session launch in the same window
is starting on, or waking, that same sandbox. A launch that wakes a suspended host now waits while another Suspend,
Resume or host operation finishes instead of failing with "This host is busy", and fails at once with a clear message if
the host is suspended again while it waits, instead of after a minute with a timeout.

This covers Suspend, Resume and Stop and suspend from a sandbox session's row menu or the top of an open session, and
Suspend and Resume in Hosts. Before, any of them kept every launch and host change in that window waiting until it
finished, which for a Resume could take up to a couple of minutes. The launch's wait shares the minute it already
allowed for the woken host to connect; the accepted Resume request itself does not count against it. The same applies
to a launch that wakes a paused container host.
