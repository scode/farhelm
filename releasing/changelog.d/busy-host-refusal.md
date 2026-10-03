---
kind: fixed
---

Stopping, restarting or renaming a session, opening or closing a tab, browsing folders and searching repositories no
longer freeze typing on a host that is busy with many session operations at once. Each stop, restart or delete can take
several seconds while a session's programs are given time to exit, and once eight were in progress on one host, one more
of these requests made every keystroke to every session on that host wait until one of them finished. Now such a request
is turned away at once with "this host is busy with other session operations; try again in a moment", before it changes
anything, so trying again is safe.
