# A few tabs that cannot be closed stop all automatic tab cleanup on a host

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If a few exited terminal tabs on a host keep failing to close, Farhelm can stop closing any other exited tabs on that
host, and logs a warning every two seconds.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F25 / COR-TAB-REAP-BUDGET`, tagged **possible**. Anchor and title:
`crates/farhelm-supervisor/src/service/ticker.rs:1805` — the per-tick tab-reap budget is spent on failed closes, so a
few persistently failing tabs block all other reaping.

A session can have extra terminal tabs besides the agent. When a tab's shell exits, the supervisor closes ("reaps") the
tab automatically, as SPEC.md promises. The reaping happens in `reap_dead_tabs` (`ticker.rs:1786-1866`) on the
supervisor's periodic tick, with a budget of 4 close attempts per tick (`REAP_BUDGET_PER_TICK`) so one tick can never
become a long batch. Every attempted close spends one unit of budget, whether it succeeded or failed (`budget -= 1` at
`:1842`, before the result is examined). Only "the tab already vanished" (NotFound) is treated as benign. Every other
failure logs a WARN and the tab stays.

Sessions are visited in the iteration order of the supervisor's in-memory session map, which stays the same as long as
the set of sessions does not change. Within a session, tabs are visited in window order. So the same failing tabs come
first on every tick. A close can fail before it removes the window, for example when Farhelm cannot read whether the tab
has its own systemd scope or when the process-tree sweep errors (`core.rs:12466-12479`). In that case the tab is still
dead and still first in line next tick. With four or more such tabs, the budget is gone before any other exited tab on
that host is reached, and automatic reaping silently stops host-wide. Even a single persistently failing tab produces a
WARN every two seconds.

This is tagged possible because it depends on a close being able to fail repeatedly. A scope that can never be confirmed
or a sweep error that recurs would do it, but the reviewer did not reproduce one. Suggested change, any one of these: do
not charge non-NotFound failures to the budget (cap them separately), give each tab a backoff with warnings at
power-of-two intervals, or rotate the starting point with a cursor so failing tabs cannot always go first.
