# Supervisor Stop and Restart cleanup

## What this was about

When Restart failed after a session disappeared from Farhelm, it could leave processes running with no session to stop
them from. On Linux, a scope tracks the processes for one launch. A scope that looked empty could still receive a
late-starting process. The plan also asked for Stop to settle a pending launch before reporting success.

## Things you should know

- On Linux, a failed Restart now records cleanup for the exact attempt when the session row is gone. At the next
  supervisor startup, it settles the recorded host start before trusting a scope check. If a session or an outstanding
  Create has since claimed that same session ID, cleanup checks the old scope without sweeping processes from the
  restored session. If the restored row owns the same scope, the cleanup stays pending.
- I returned the Stop item to TODO under the plan's complexity rule. The existing Stop intent can be marked complete
  when the terminal goes missing, or cleared when the supervisor rediscovers a live host. Fixing the Stop reply alone
  would still let that intent disappear before the sweep was confirmed.

## Open questions and possible follow-ups

The Stop follow-up needs to preserve an unfinished sweep through supervisor restart and offer consent based on whether
the agent could still run. The TODO now records those constraints and the related case where a terminal is usable even
though the create confirmation could not be written.

## Pull requests

- [#5005](https://github.com/scode/farhelm/pull/5005/changes) keeps a failed Restart's cleanup retryable and updates the
  remaining Stop TODO.

## Checks run, reused and skipped

- `cargo nextest run -p farhelm-supervisor -p farhelm --lib --test e2e -E <selection>` ran 20 focused cases at
  `b55b54a8` (recorder `454e175f-41f0-417d-941d-4b9354eb0ea8`). Nineteen passed across Restart cleanup, startup reclaim
  and session ID ownership. The late-start case timed out at its stale socket, and that failure is retained. The earlier
  attempt at `95caa576` stopped on a compile error before running tests (recorder
  `848801f5-a7d5-4118-9efa-79ae14b6d9b1`).
- `cargo nextest run -p farhelm-supervisor --lib -E 'test(=service::core::host_restart::tests::pending_restart_cleanup_checks_its_starter_before_its_scope)'`
  passed the repaired fixture at `c9135d78` (recorder `67bb397a-252a-463a-8c5d-7256a9e498ab`). That change touched only
  the fixture, so the other nineteen passing results still apply. These scope checks use a stand-in manager. The
  selected e2e checks drive the real supervisor and terminal hosts.
- `cargo clippy --locked -p farhelm-supervisor --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  `dprint check SPEC_impl.md TODO.md releasing/changelog.d/retry-failed-restart-cleanup.md`, `git diff --check`,
  `python3 releasing/check-changelog.py format` and `python -B scripts/check-test-sleeps.py` passed. These direct checks
  have no recorder IDs.
- I skipped the workspace suite and visual checks. The change is in the supervisor's startup and Restart paths, and the
  focused selection exercised their affected boundaries.
- Main advanced with the quick dialog work and queue metadata. It changed no supervisor code or relevant tests. The
  integration trial merged cleanly, so the selected test results still apply to the draft.

## Review gate

An independent source review identified a second-stage race after host shutdown, a missing private socket directory
check, misleading scope-only failure wording, and two fixture gaps. The code now settles the exact recorded start before
retiring it and refuses unsafe host directories. The recheck cleared the production changes and reservation case. The
final test-only change proves that the scope is absent before the starter is released and seen on the next attempt.

### Landing

#### What landed with it

This plan landed second in its round, right after the What's new restyle (`launcher-style-whats-new`), which changes
only the app and the release notes checker and shares no code with it. Since the plan's PR was based, main gained the
quick switcher, template list and Rename restyle and the host provisioning fixes for Tailscale Uninstall and SSH sign-in
priority; none of them touches the supervisor, and the rebase applied without conflicts. No schema or protocol version
changed on either side.

The report's first focused run had one Restart cleanup case time out, which the executor fixed in the test's fixture and
re-ran. The landing's workspace run covers every supervisor test, that one and the plan's others included, and all of
them passed.

#### Fixes made while landing

All inside [#5005](https://github.com/scode/farhelm/pull/5005/changes), documentation only; the landing's independent
review found no blocker.

- **SPEC_impl.md still described the old rule in one of its two places.** Its Restart paragraph said the replacement's
  host record can go once its leftover scope cleanup was saved; the plan keeps the record with that saved cleanup, as
  its own amendment to "Cleanup on quit" says. The Restart paragraph now says the same.
- **The startup reclaim of orphaned terminal hosts did not document its new exception**: a launch record that a pending
  Restart cleanup names, and its socket, are left to that cleanup's retry, since both run at startup and only one may
  settle a record. Its module docs now say so, and the Restart code's docs say that the new check of a failed launch
  also runs for an ordinary failed Restart.
- **The changelog draft read as if the startup retry were new.** That retry shipped in v0.44.0; the draft now says what
  this plan adds and gives the curator that context.

#### Checks

Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. All the checks below ran
on both plans' PRs stacked in queue order on main as it stood at the start of the landing (`3236d5b7`), which applied
without conflicts; every file's change was exactly what the executors pushed until the landing fixes, which this round
made inside the plans' own PRs: #5004 and #5008 for What's new, and #5005, documentation only, for the Restart cleanup
fix.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `4c1d2119-e93c-4914-8853-ee0f72a604fc`), before the fixes: all 8726 passed, with no runtime `SKIPPED` lines. This
  includes the GUI crate's tests and every supervisor test, the Restart cleanup and orphan reclaim tests among them. The
  fixes since then change one What's new helper to an equivalent form, a checker comment, and documents and code
  comments (the supervisor's among them), so this run still applies.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: the first run
  failed on the What's new helper; after the fix all four passed.
- After the fix, the What's new tests, `cargo nextest run -p farhelm-gui --lib -E 'test(/whats_new::/)'`, on the test
  worker (recorder run `410e532f-a1dd-4630-bfe7-e9d08bafd6e2`): all 5 passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `0c6809d4-fc0c-4d4d-984f-6733e66273db`), before the fixes: 50 steps passed, one skipped by design on Linux (paste).
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `a7a2a9b6-2f76-4259-8cc2-c641e5b2cb98`), before the fixes: passed, including the two What's new pictures the plan
  updated.
- The screenshot batch tool's test, including the committed scene list (recorder run
  `805b2d42-fadf-473c-87ff-a16fc3f1a3c1`), locally, before the fixes: passed.
- `python3 releasing/check-changelog.py --self-test` and `python3 releasing/check-changelog.py format`, locally, before
  and after the fixes: passed.
- `python3 scripts/check-agents-md.py`, `cargo fmt --all -- --check` and `python -B scripts/check-test-sleeps.py`,
  locally: passed. `dprint check`, locally: these plans' files pass; it fails only on over-long lines in the triage
  outcomes archive, which main has had since an earlier triage record (#4978) landed. No PR fixes them yet; a
  formatting-only one would, and I recommend it, as earlier rounds' notes did.

Reused: the workspace, smoke, screenshot comparison and scene list runs above across the fixes, for the reason given
(nothing the fixes change draws differently). The executors' runs were focused selections on older bases, and everything
they covered ran again above.

Skipped: the tests of the desktop app's outer shell (`cargo nextest run -p farhelm-desktop`), since the What's new
window lives in the app's shared GUI crate, whose tests ran in the workspace suite, and nothing in the shell changed; a
Mac run, since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing.

#### Things you should know that the report does not say

- **A recorded host that still answers stops the startup retry for good (the review's should-fix, left as a
  follow-up).** It takes this sequence: Restart finds the session deleted, ending the replacement agent's processes
  fails, and ending its new host fails too. On every later start, the retry then waits for that host, the agent's scope
  is never cleaned, and the leftover cleanup is never retired, while the orphan reclaim now leaves that host to the
  retry. Before this plan, startup still cleaned the scope in that case. A host that answers has already started its
  agent, so the race the plan guards against no longer applies to it; cleaning the scope but keeping the record and the
  saved cleanup would be safe. It only happens after ending the host already failed, so I landed the plan and recommend
  that change as a follow-up; say if you want a TODO entry.
- **Smaller points the review noticed.** With several leftover cleanups, the later ones get less of the startup time set
  aside for them, which only delays them to the next start; a session restored under the same ID with the same run logs
  a warning on every start until it restarts or is deleted, which nobody can act on; and one test names the system
  Python by its full path. None needs a follow-up on its own.
