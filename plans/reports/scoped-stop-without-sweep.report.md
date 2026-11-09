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

### Landing

Landed together with per-host-checkout-folder (checkout folders in host settings), stacked below it, on a main that had
just gained stop-start-message (a Stop during an agent's startup is now reported as such), the update floor for old
installations and hosts, and steadier terminal sizing on session switch. stop-start-message changed the same stop paths
in the supervisor. A fresh-context review read the two against each other and found no conflict:

- A Stop during the login shell, before the agent's systemd scope exists, still finds the session's terminal process
  alive, so cleanup falls back to the full scan of every process on the machine (the "sweep").
- A Stop after the scope starts ends Farhelm's small launch program inside it along with the scope.
- The "stopped before the agent finished starting" report reads only the launch's own files, after cleanup, not anything
  the sweep finds.
- The order of the steps and the locks are unchanged.

When this plan was blocked at its first landing, you decided that cleanup must keep the sweep when a tmux server runs
inside a scope it would trust: stock tmux moves each pane's programs into their own systemd groups, so trusting the
scope alone would let pane programs outlive the session. You also decided Delete must keep the sweep when it cannot list
all of a session's terminal tabs. The review confirmed both are done, with the tests you asked for. A real stock tmux
test shows a pane program that ignores the hangup signal still ends with Stop. The stale comments are updated, and the
specs agree.

#### Fixes made at landing

- Five more comments and one log message still said the sweep always runs, including one stop-start-message had just
  added (#3237).
- SPEC_impl.md now says outright what the review found. A scope systemd no longer has loaded reads as empty, which the
  tmux check cannot tell from unreadable, so it keeps the sweep. The cheaper cleanup therefore applies only while every
  scope involved is still running. Deleting a session whose agent or a tab already exited, a Stop or Restart after the
  agent exited, and a containerized Delete always sweep (#3237).

#### Things you should know

- Because of that, the saving is smaller than "every scoped session". The changelog entry says the scan is avoided "when
  its systemd groups confirm cleanup", which stays true, but a reader may take it to cover more cases than it does. The
  executor's report describes the saving the same way.
- From the review's reading of the code, not observed: a tmux server started in the instant between the tmux check and
  the scope kill would leave its panes' hangup-ignoring programs running; before this plan, the always-run sweep would
  have caught them. Separately, a Stop may record fewer of the session's exit codes at the moment of stopping; they are
  filled in later, so nothing is lost. Neither seems to need a follow-up.

#### Checks

The two plans were checked together, stacked in queue order on main, on this development machine and, at the
maintainer's request, on Tensorlake sandboxes. The tree that merged is the one checked last (`80366b10`, `ef23035c`);
main after the merges equals it outside `plans/`.

On the stack before the landing fixes (`2cc53615`, `160f30e2`):

- Here: `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`, the test-sleep
  checker, `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo check -p farhelm-ui --features desktop` and the docs website build passed.
- Tensorlake, browser: `host-checkout-folder`, `github-checkout-composer`, `settings` and `spawn` on Chromium and WebKit
  over six sandboxes (recorder runs `15407382`, `88777727`, `07584a58`, `8a2508fb`, `bb673a4a`, `8c459a18`). One test
  failed: the WebKit Settings navigation test's check that the Checkouts section fits without scrolling (run `8c459a18`,
  retained). per-host-checkout-folder's landing fix to its help text addresses it; every other test passed.
- Tensorlake, Rust: every Rust test in the workspace except the desktop app's, from one archive over nine sandboxes
  (runs `67cc46ef`, `271aa0c5`, `cf6e2b79`, `5c0fcb65`, `8dfee46b`, `14d0ff68`, `b9891b30`, `79ba2bd9`, `385cde9f`),
  with two failures.
  - `attribute_compile_contract_matrix` compiles small crates with a nested `cargo`, which the sandboxes do not have.
    Neither plan touches its crate (`farhelm-testtrace-macros`); it passed here last round on a tree that differs from
    this one only outside that crate (run `05991d87`), so that pass is reused.
  - `create_idempotency::a_crash_before_the_outcome_commit_still_replays_the_same_session` (run `cf6e2b79`, retained)
    had Delete refuse because the sandbox's per-user systemd manager stopped answering its probes, which is the
    situation that refusal exists for. It passed five times in a row here (run `84880653`, in the recorder's plain
    command mode, since its nextest mode refuses repetition). It also passed on a rerun from the same archive on three
    sandboxes, three times each, together with the whole create-idempotency and session-lifecycle modules (runs
    `41f38188`, `1ad1e77f`, `3aa87de6`). In that rerun, a different test on one sandbox reported the same manager
    unusable and skipped. This is one observation of the sandbox's manager, not a flake of the test.
  - The sandboxes have no stock tmux, so the new stock-tmux test skipped there; it ran here (next item).
- Here, on the same stack: recorded `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e` filtered to
  the scoped Stop test, the scoped login-shell Stop test, the stock-tmux test and stop-start-message's early-Stop tests
  (`pending_stop_*`): 9 passed, none skipped, run `e5d5c56f`. This machine has a stock systemd-linked tmux and a working
  user manager, so the stock-tmux test really ran.

After the landing fixes: `cargo fmt --all -- --check`, `dprint check`,
`cargo clippy -p farhelm-supervisor -p farhelm-ui -p farhelm --all-targets -- -D warnings`, a fresh build and release
web build, and on Tensorlake `settings` and `host-checkout-folder` on both engines: 74 passed, runs `e107e43d`,
`90807ce6`, `94c646d1` and `6d9da565`. The Rust tests were not rerun after the fixes: in the supervisor they changed
only comments and the text of one informational log line, which no test reads. The browser runs used the recorder's
plain command mode (its Playwright mode cannot pick one engine per file) under the sandbox's slightly older Node 24;
every sandbox reported its per-user manager and SSH to itself working.

Reused: only the compile-contract pass above. Everything else that matters was run on this round's stacked tree, which
includes stop-start-message, so the executors' own runs on their older bases are not relied on.

Skipped: the rest of the browser suite (the specs above cover the Settings, checkout and launch paths
per-host-checkout-folder changes; the cleanup change has no browser surface beyond what the Rust end-to-end suite
drives), the desktop smoke (no desktop-only code changed beyond what the desktop compile check covers) and hosted CI
(manual-only, not dispatched).
