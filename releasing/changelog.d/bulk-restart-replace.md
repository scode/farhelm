---
kind: added
---

When every session you have marked in the list has stopped or ended, for example after a remote host rebooted, a new Restart/Replace button beside Stop starts them all again in one click. Sessions that can resume their conversation are restarted, and the rest are replaced with a fresh session using the same settings.

Nothing asks first. Sessions on the same host start one at a time, while different hosts go in parallel. A session that still has a terminal tab open (or a program that may still be running) is left out rather than replaced, since Replace would close it without asking; its row says so, and its own Replace confirms as usual. With Skip session confirmations on, such a session is replaced like the others. Sessions whose sandbox or container is suspended, stopped or being deleted are left out with a reason on their row, and the button waits while a host is being added, set up, updated or removed.

For the release curator: the plan's release note names the button "Restart"; it is labelled "Restart/Replace", with the hover help "Restart when possible, Replace when not", as the plan's decisions settled. The note also does not mention the open-tab exclusion above.
