### What this was about

Removing a remote host could stop its supervisor and then refuse every Retry. One cause was an absent session-server
socket whose diagnostic differed on hosts using uutils or BusyBox tools. Another was an interruption after Farhelm's
private tools had been deleted: tmux left its socket behind, and Retry tried to run the deleted client against it. The
maintainer delegated small fixes for these cases and for an uninstall test that assumed Linux tools on macOS.

The stop step now accepts the precise supported missing-socket diagnostics while retaining permission and I/O refusals.
After successful or explicitly excused tmux termination, it removes the socket before the private client can be deleted.
The local command fixture is Linux-only, with its tool assumption documented.

### Things you should know

The retry fix prevents new stranded removals. Hosts already stranded by an older release, with both a stale socket and
the private client deleted, remain outside this fix and may need manual recovery. That limit was explicitly accepted in
the triage decision. No uninstall protocol, persistent state, or database schema changed.

The real systemd user-manager and localhost SSH regression ran successfully; it was not an early-return skip. It now
checks the owned tmux process exits, the socket disappears without manual removal, and a retry works with no client. The
uutils and BusyBox cases exercise controlled diagnostics through the production shell script; they are not tests on
separate distributions. Native macOS was unavailable; the platform gate was checked in source review.

### Open questions and possible follow-ups

None required for this plan. Recovery of hosts already stranded by older releases was explicitly excluded.

### The PRs

- [#2096: remote uninstall with uutils stat](https://github.com/scode/farhelm/pull/2096/changes) — draft; supported
  absence diagnostics, refusal coverage, and removal of the matching TODO.
- [#2100: retry interrupted remote uninstall](https://github.com/scode/farhelm/pull/2100/changes) — draft; socket
  removal and real retry regression.
- [#2102: Linux uninstall command fixture](https://github.com/scode/farhelm/pull/2102/changes) — draft; Linux gate for
  the command fixture. The three PRs form a linear stack in that order.

### Checks run, reused and skipped

- Ran recorded nextest checks with the pinned runner and tmux, four slots and zero retries. Run
  `a4d1bb0e-be96-4a1d-8e27-868e710824dd` passed both the new stat regression and the existing command fixture.
- Run `c98df562-ef62-462f-b0e2-ee0b25a37bed`, selection `test(provisioning::) & test(uninstall)`, passed all 15 selected
  tests, including the real user-manager/SSH retry. Read the retained selected-case output; no runtime setup was
  skipped.
- Final platform-gate fixture run `883a1a93-6211-4fb7-9fa9-7bf7bd4fee7e` passed its one selected test on Linux, with no
  runtime skip.
- Ran `cargo clippy --all-targets -- -D warnings`, formatting, changed-file dprint, changelog format validation and the
  isolated test-sleep checker at each review unit: all passed, including the final stack. The sleep checker inspected
  301 delays and found zero without a rationale.
- A temporary pinned-tmux experiment confirmed that its socket remains after successful server termination, before
  implementing the socket fix. Earlier runtime evidence is reused for the final test-only platform restriction because
  it does not change the stop script or the Linux test body. Ledger-only PR-link updates do not invalidate it.
- Reviewed main's intervening adoption-identity renderer change and bookkeeping. They do not interact with remote
  uninstall or its fixtures, so no rebase or additional runtime run was needed for that delta.
- Skipped browser, website, installer and unrelated runtime suites: the changed behavior is the remote stop command and
  its fixtures, covered by the selected uninstall tests. Native macOS execution was unavailable.

### The review gate's outcome

All three required fresh-context gpt-6.1-sol/high source reviews found no issues; their full findings were read and
accepted. Every change stayed within its recorded complexity gate. Valid wording cold reads passed; an invalid blind
read for the last title was replaced and is excluded from the evidence.
