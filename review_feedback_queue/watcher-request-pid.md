# Plans watcher deadline can signal a recycled request PID

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Plans watcher deadline can signal a recycled request PID.

## Details

`F39 / COR-WATCHER-REQUEST-PID` — **possible** — `scripts/plans-watch.sh:194` — Plans watcher deadline can signal a
recycled request PID

The plans watcher limits each GitHub request or queue check with a separate watchdog. That watchdog remembers the
request's numeric process ID, waits for the deadline, and sends it TERM. Meanwhile, the parent waits for the request to
finish and only then cancels the watchdog. If the request finishes near the deadline, there is an overlap in which the
watchdog can still signal a number whose original process has already been reaped.

Process IDs can be reused. If another process under the same account receives that number before the watchdog signals
it, the timeout could stop unrelated work. Neither reuse in this overlap nor a wrong-process signal was observed; this
is a possible ownership race, not a demonstrated kill. Put the deadline and child ownership in one helper, and retain
the child's identity until the timeout can no longer signal it. Proposed bucket: highest. No possible cover was
identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_general p1`, `auto_security p1`, `auto_secrets p1`, `auto_edges p1`.

Possible cover recorded during collection: none identified.

Collection caveats: PIDreuse during overlap unobserved, no wrongprocess kill reproduced.

## Filed reviewer metadata

- `auto_general p1`: confidence as filed: possible / likely only if the old request PID is reused before the watchdog is
  stopped. Severity: process loss. Suggested bucket: highest. Suggested bucket as filed: highest.
- `auto_edges p1`: confidence as filed: possible; request PID reuse during cancellation is unverified. Severity: loss of
  unrelated processes/work. Suggested bucket: highest. Suggested bucket as filed: highest.
- `auto_security p1`: confidence as filed: possible / likely with the unverified premise that the request exits/reaps
  and its PID is reused while the timeout watchdog reaches its signal before the parent cancels that watchdog. No
  reproduction. Suggested bucket as filed: highest. Confidence: possible / likely with the unverified premise that the
  request exits/reaps and its PID is reused while the timeout watchdog reaches its signal before the parent cancels that
  watchdog. No reproduction.
- `auto_secrets p1`: confidence as filed: possible; confirmed cached-ID mechanism, unverified premise that the operating
  system reuses the request PID after the parent reaps it and before the watchdog is cancelled. Suggested bucket as
  filed: highest.
