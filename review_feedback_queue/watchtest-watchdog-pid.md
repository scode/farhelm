# Watcher test deadline can signal a recycled fixture PID

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Watcher test deadline can signal a recycled fixture PID.

## Details

`F43 / COR-WATCHTEST-WATCHDOG-PID` — **possible** — `scripts/test-plans-watch.sh:114` — Watcher test deadline can signal
a recycled fixture PID

The plans-watcher test harness gives each fixture a thirty-second deadline using a separate watchdog. The watchdog
stores the fixture's numeric process ID and sends TERM at the deadline. The test parent waits for the fixture to finish
before cancelling that watchdog, leaving the same possible overlap as the production watcher: the fixture can be gone
while the watchdog still has permission to signal its old number.

If completion occurs near the deadline and the operating system reuses the ID in that overlap, an unrelated command
under the same account could receive TERM. That schedule and reuse were not verified. Use a timeout helper that owns the
child and retains its identity until watchdog cancellation is complete. This affects the test tool, not a running
Farhelm session. Proposed bucket: highest. No possible cover was identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_security p1`, `auto_secrets p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Unverified completion-neardeadline PIDreuse.

## Filed reviewer metadata

- `auto_security p1`: confidence as filed: possible / likely with the unverified premise of child PID reuse between
  completion and watchdog cancellation. No reproduction. Suggested bucket as filed: highest. Confidence: possible /
  likely with the unverified premise of child PID reuse between completion and watchdog cancellation. No reproduction.
- `auto_secrets p1`: confidence as filed: possible; unverified premise that a test child exits near its thirty-second
  watchdog deadline and its PID is reused after reaping, before watchdog cancellation. Suggested bucket as filed:
  highest.
