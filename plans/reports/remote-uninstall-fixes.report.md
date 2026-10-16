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

### Landing

Landed as #2096, #2100 and #2102, bottom-up, third of the round's nine plans, tested stacked with the others on main.

#### Review

An independent fresh-context review found nothing blocking. Absence is accepted only from messages that end in the "no
such file" error for GNU coreutils (including its statx spelling), uutils and BusyBox, so permission, I/O and
missing-command errors still refuse and an existing socket can never be reported absent. The socket is removed only
after its server stopped or was excused, so a retry finds it gone and finishes; the scripts are valid POSIX shell and
quoted correctly. The removed TODO entry is exactly this plan's.

#### Fixes made while landing

Changelog wording: both fragments now say what the user sees (removing a remote host no longer fails on every retry on
hosts whose tools come from uutils or BusyBox; an interrupted removal can be finished with Retry) instead of naming
sockets and stat.

#### Left as is

No test covers a BusyBox-style refusal message; a directory at the socket path now keeps the host instead of being
treated as absent, which is the safer direction.

#### Checks

Run on all nine of this round's plans stacked on main (terminal-link-safety, ntfy-token-server-change,
remote-uninstall-fixes, update-failure-reporting, docs-accuracy-fixes, host-picker-dotted-i,
setup-advice-state-directory, copy-guidance-resize, brick-test-feedback-evidence), with every landing fix in:

- `cargo fmt --all -- --check`, `dprint check`, the changelog lint and the sleep checker (301 delays, none missing a
  reason); `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`, the UI crate's
  clippy with the desktop feature and `cargo check -p farhelm-desktop`; the brick-test tooling's own test.
- Through the recorder: the protocol and helm packages in full with pinned tmux, run
  `6352fad3-a3b1-48dc-9884-3efc17ee03c6`: 1258 passed; the CLI's own unit tests, run
  `0e1855d7-6f43-4da7-8a9a-495936ae565e`: 171 passed; the UI crate with the desktop feature, run
  `513addbb-2505-4c54-bc07-2ac2ec06713b`: 655 passed.
- The JS harness (257 passed) and the website build.
- The terminal-link-clicks, terminal-links, terminal-files, ntfy-settings, provisioning, settings and tooltip-coverage
  specs on Chromium and WebKit, run `97a26936-2b7b-42a5-af59-8f0fdb0b9d78`: 208 passed, 2 failed. Both failures were the
  clipped-disclosure test pinning the refusal's old layering after the quick switcher closed (see terminal-link-safety).
  After that expectation was fixed, the terminal-link-clicks spec on both engines, run
  `a698ea79-451d-4dd4-87f5-24ef1d4b1cad`: 16 passed. An intermediate run, `6113fd52-60bf-4730-ae85-5e9dc2e90d46`, failed
  on WebKit only because the first corrected expectation assumed Chromium's behavior.
- Not run: the supervisor package and the CLI's end-to-end targets, which no PR in this round changes; other browser
  specs.
