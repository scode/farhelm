# Scoped cleanup avoids the machine-wide process scan

## Since the last review

Draft #3237 was amended; no PR was added or dropped. The landing review found that ordinary stock tmux moves pane
programs out of its server's systemd group, so the original optimization could leave them running. The maintainer chose
to retain the full scan when a tmux server is inside a group cleanup would trust, and when Delete cannot discover all
its tabs. This amendment implements both exceptions, their regression tests, and the specification/comment corrections.

## What this was about

Stopping a session, closing a terminal tab or deleting a session scanned every process on the host even after its
systemd groups had been emptied. The maintainer chose to trust those groups on Linux and keep the full scan for unscoped
or uncertain work. Cleanup omits that scan only when containment is complete, every named group is confirmed gone, no
explicitly tracked process survives, and a check before the first group is killed rules out a tmux server.

## Things you should know

A tmux server started by an agent or tab still ends with it, along with ordinary pane jobs that tmux moves into other
groups. Unavailable, capped or unreadable membership checks retain the full scan. Versioned tmux executable names are
treated conservatively when their kernel role name is truncated.

Delete also keeps the scan without an agent terminal or when an old tmux session has disappeared. A session moved from
tmux to a new terminal can still have old tmux tabs. Existing records cannot distinguish that history from a restarted
session that always used new terminals, so Delete conservatively scans both when their tmux session is absent. Fresh
sessions in new terminals keep the fast path when the other conditions hold. No new history database was added.

Programs that deliberately move themselves into another group, or merely carry a session's environment marker without
belonging to its group, remain outside scope-only cleanup. This is the accepted tradeoff, documented in the specs and
changelog. Terminal handling, container shutdown and cleanup refusal rules are unchanged. Extra containment metadata
that cannot be read retains the scan rather than introducing a new refusal.

## Open questions and possible follow-ups

None blocks this plan. The performance benefit remains conditional: a login shell that leaves a live explicit process
outside the group still needs the full scan. Shell startup and rare interrupted-launch recovery were not redesigned. No
latency benchmark was required or run; the tests distinguish omission of the scan itself from merely finding nothing.

## The PRs

[#3237](https://github.com/scode/farhelm/pull/3237/changes) — cheaper confirmed-group cleanup with the tmux and
incomplete Delete exceptions; remains a draft for the landing monitor. Verified head `bb7074e0`.

## Checks run, reused and skipped

- Two recorder-backed `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E …` selections passed on
  Linux with Rust 1.99.1, pinned nextest 0.9.148, pinned harness tmux 3.7c, four slots and zero retries. Both built
  their binaries from the same frozen working tree (`a32cf907`, based on `9726aa8a`), with matching complete source
  fingerprints; the pushed amendment has exactly that tree.
- Run `872b7dca-df27-4aef-bfd4-a4b5edc643b0`: seven cases plus fixture setup passed. These cover the tmux role
  classifier, inspection before every group kill, no-scan fast path, both missing-discovery Delete cases,
  migrated-terminal history, and real stock tmux 3.4. That last case asserts its server is inside Farhelm's group, its
  live marked pane job is outside in a tmux-created group and ignores SIGHUP, and both are gone when Stop returns.
- Run `4b41ce98-98e3-4228-9d43-d89c8349203b`: 62 cases plus fixture setup passed, comprising 51 supervisor and 11
  integration cases. The selection covers the sweep and host-selection modules, tab scope/re-probe and dead-tab cleanup,
  bounded membership parsing, Delete enumeration/re-probe/refusal/retry, real scoped Stop/tab close/Delete, host Stop
  and Restart, and forced-unscoped marker cleanup. Some fast-path cases overlap the seven-case selection. Both commands'
  complete retained output has no runtime `SKIPPED`; unrelated selected-out cases are not coverage.
- Initial run `cd32b085-b559-4b2e-830c-ea8a87505451` failed to build because a new test assertion required an
  unsupported error format. No test ran. That failure is retained; the assertion was corrected before the successful
  commands.
- `cargo fmt --all -- --check`, `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, changed
  Markdown `dprint check`, `python3 releasing/check-changelog.py format`, the isolated test-delay checker (430 delays,
  none unexplained), and whitespace checks passed.
- Reused across the final fold and commit-message rewrite: the successful checks above, because the trees are identical.
  The final fetch found no new main changes. No previous round's runtime result is needed to establish these
  corrections.
- Skipped the full Rust battery, browser, desktop, installer and provisioning suites: focused cleanup checks cover the
  affected behavior. No macOS runtime run was made; its existing portable cleanup remains selected. The new portable
  test fixture uses a custom executable because modern macOS hides Apple platform binaries' environment markers.
  Container teardown and its ordering did not change. Hosted CI was not dispatched.

## Review gate's outcome

The required single fresh gpt-6.1-sol/high reviewer found two issues: versioned tmux names could hide the server role,
and the initial Delete fixture's marker would be unreadable on modern macOS. Both were corrected and rechecked in that
same review; no actionable finding remains. Fresh scope assessments approved the bounded observation and conservative
discovery correction without a new containment or history subsystem. The separate documentation pass and commit/PR
wording cold read are complete.
