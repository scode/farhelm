### What this was about

Before the real-agent drift tests run, the drift runner upgrades each agent CLI with the vendor's own updater (or the
vendor's installer when the updater is known to break). An updater can leave a background worker running after the
command itself returns, and one that detaches into its own session escapes the runner's kill on a timeout. Such a worker
could replace the CLI's binary while the runner checks its version or while a drift case runs, so the case's verdict
would name the wrong release. Nothing user-facing in Farhelm changes: this is the maintainer-run drift tooling.

You decided the containment should be a transient systemd user scope (a cgroup, which a detached process cannot leave)
rather than making the runner a subreaper.

What was built: every updater and fallback installer now runs in a scope of its own with a unique name. When the updater
exits, the runner waits, within the same ten-minute upgrade limit, for everything it started to finish, so a legitimate
worker completes its swap. Anything still running at the limit, or an updater that overruns it, is stopped and confirmed
gone, and that upgrade fails. Ctrl-C or SIGTERM during an upgrade stops the scope first and then ends the run with no
case run. If the scope cannot be confirmed stopped, the whole run stops there rather than go on beside a possible
survivor. Without a systemd user manager no updater runs and every upgrade fails saying so; as with any failed upgrade,
the cases then still run against the versions already installed, and the run exits nonzero with an UPGRADE FAILED line
naming each agent, so the stale verdicts cannot pass for current ones.

### Things you should know

- An updater that starts a long-lived process on purpose (a daemon it expects to keep) would now make its agent's
  upgrade wait the full ten minutes and fail. None of the current agents is known to do that, but the real vendor CLIs
  were not run for this plan; the first real drift run after this lands is worth watching for it. The drift test guide
  says so.
- The runner's own tests of the upgrade step (version reporting, closed input, installer order, and the new containment
  tests) now need a systemd user manager. On a machine without one they print SKIPPED, which counts as missing coverage;
  the validation docs say so.
- A limit this plan leaves in place (not one you decided on before): a process stuck in an uninterruptible wait survives
  even the scope's final SIGKILL until that one system call finishes. If that call is the binary swap itself, it can
  land after the runner moved on. The code documents this; detecting it would need reading the kernel's cgroup state
  directly. Say if you want that built.
- The test of a signal arriving the instant the updater starts shows the cancellation is handled cleanly, but on its own
  it cannot show a running scope being stopped (the worker has usually not started yet); the SIGTERM test does.
- This is typed as a test-tooling change, so it carries no changelog fragment. It removes the TODO entry "Stop the agent
  updater's process group before running drift cases."

### Open questions and possible follow-ups

- No open questions.
- Possible follow-up, only if it happens: if a vendor updater turns out to keep a daemon on purpose, its recipe would
  need a way to say which process may stay; that is not built.

### PRs

- #5093 test: contain drift upgrades in a transient user scope

### Checks

Run now, on the PR's final revision:

- Recorder run `cc4de118-25d3-416c-bf35-39e36b650440`: `python3 -B scripts/test-run-harness-drift.py`, 31 tests passed,
  none skipped, and no upgrade scope left running afterwards. The new tests start stand-in updaters whose workers detach
  into their own session and check those workers by their own process ids: a worker that finishes after the updater
  exits is waited for; one still running at the limit, or behind an updater that overruns it, is stopped with it and the
  upgrade fails; SIGTERM stops everything before the cancellation ends the run; a scope that cannot be confirmed stopped
  ends the run with no installer, version check or further agent; and with no user manager, no updater is started.
- Recorder run `ac02cba5-12c2-4886-9af2-534a7c751475`:
  `python3 -B scripts/test-run-harness-drift.py ScopedUpdaterTests`, the scope tests against a version with the wait and
  the scope stop disabled on purpose: four of the five failed; the fifth is the start-signal test described above.
- `python3 -m py_compile` on the upgrade module, the drift runner and its tests; `dprint check` on the changed docs;
  `python3 scripts/check-agents-md.py` (0 errors).

Reused: none.

Skipped: the real drift run, which spends vendor turns and upgrades the installed agents, and is only run when you ask
for it; and every Rust, GUI and installer suite, since nothing outside the drift tooling changed.

### Review gate

One fresh-context Opus 5.5 reviewer at high effort, two rounds. Round 1 found one real gap: a scope that could not be
confirmed stopped was reported as an ordinary failed upgrade, so the fallback installer, version check and cases could
still run beside a survivor. That is now the run-ending case above. It also found a late signal that could be lost, the
validation docs understating which tests need a user manager, a missing entry in the full-test list, and tests that did
not first check the stand-in worker was really running detached; all fixed. Round 2: no remaining correctness findings;
its two wording and edge-case suggestions were applied.

### Landing

#### What landed with it

This round landed three plans before this one (a session's menu on right-click, the launcher's late wake failures, and
Stop refusing an unconfirmed start); see the first plan's notes. They change app, helm and supervisor code; this plan
changes only the drift test tooling and its documentation, so nothing interacted. Since it was built, main had gained
only two supervisor and helm changes, neither in the drift tooling. The landing's cross-plan review checked that the new
scope's name (`farhelm-drift-upgrade-<id>.scope`) matches none of the names the supervisor or any cleanup script looks
for, so nothing in Farhelm will mistake it for a session's.

This plan also edits the validation inventory and the run-all-tests list, as the repository's rules require when a test
script changes; no other plan this round touched those files. The drift runner's own tests passed on the landing
machine, whose systemd user manager they need.

#### Fixes made while landing

None for this plan. The reviewers found nothing to fix.

#### Worth knowing

- **A drift runner that dies abruptly leaves its updater running.** If the runner is killed with SIGKILL, or its
  terminal closes, nothing stops the updater it was upgrading with. Because that updater runs outside the runner,
  stopping the Farhelm session the runner ran in does not end it either. A hung updater would keep running until stopped
  by hand, and a late worker could still replace the agent's binary afterwards. A follow-up could give the scope a
  run-time limit.
- **One Ctrl-C can go unheeded.** If the scope for an upgrade fails to start and a Ctrl-C or SIGTERM arrived just
  before, the run does not stop for that signal. This is a narrower case than the lost signal the executor's review
  fixed, and that fix stands.
- **Two tests are weaker than they look.** Each checks for a timeout message but would also pass if the runner printed
  the other timeout's message, so a mix-up between the two would go unnoticed. Separately, a scope test that fails can
  leave its stand-in processes running on the machine until someone stops them.

None of these affects a normal drift run. The first is the one worth a follow-up entry if you want one; the others are
small cleanups.

#### Checks

Run once for the whole round, on its four plans stacked in queue order (session-right-click-menu,
launcher-late-wake-failures, supervisor-stop-pending-launch, drift-updater-teardown) on main as it stood when the
landing started; what reached main after each plan was built is in each plan's notes. The test worker is an aarch64
Linux machine with a systemd user manager:

- On the test worker, the four clippy configurations (`cargo clippy --all-targets -- -D warnings`,
  `cargo clippy -p farhelm --bins -- -D warnings`, and `cargo clippy -p farhelm-desktop --all-targets` with and without
  `--features agent`, each with `-D warnings`): clean.
- On the test worker,
  `cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm-gui -p farhelm-desktop`: the
  first attempt, recorded run b6571c52-9267-4c9b-9635-f7c159c2a3f1, never ran a test, because the compiler was killed
  while building the helm's tests (the worker ran out of memory with four build jobs). The rerun with two build jobs,
  recorded run 264e5b64-065f-46f0-8dcb-ab15f3ef799d: 7,624 passed, 17 skipped (tests marked ignored). That is every test
  of the shared message types, the helm, the supervisor, the desktop app and its GUI.
- On the test worker, the end-to-end tests of host creates, host terminals, relaunches, supervisor restarts and every
  test whose name contains `restart` or `stop` (`cargo nextest run -p farhelm --test e2e` filtered to them), recorded
  run 08bd628d-e968-4273-8466-14c3e4f84735: 160 passed, and none skipped for lack of systemd.
- On the test worker, the desktop app's end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), recorded run
  2a146ce3-50ae-460e-a84b-d597d7d7b659: 48 of the 49 steps passed; the paste step runs only on a Mac and was skipped, as
  it always is on Linux.
- Locally, every screenshot scene compared with its approved picture (`python3 scripts/check-gui-goldens.py`), recorded
  run 33bc1d03-4b78-4cc2-a05e-d822d0dce764: every picture matched, the new late wake failure picture included. This
  covers the screenshot re-render that the launcher-late-wake-failures report said it skipped after its rebase.
- Locally, `python3 -B scripts/test-gui-screenshot-batch.py`, which checks the screenshot scene list against the golden
  manifest: recorded run b81cb326-fd3c-4921-9f49-69ff01bb3f0d failed because the new late wake failure picture was
  missing from the scene list; after the landing fix in launcher-late-wake-failures, recorded run
  ffecfaab-79e4-4bfd-942e-a25925a47bd5 passed.
- Locally, the drift runner's own tests (`python3 -B scripts/test-run-harness-drift.py`), recorded run
  029c1d18-8368-4a93-b71c-ea765289da54: 31 passed, none skipped, on the landing machine (Linux, with a systemd user
  manager).
- Locally, `python3 scripts/check-agents-md.py` (0 errors; its one warning is an old one about the frozen triage
  archive), `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed.

Reused: nothing. Skipped: the Podman suites, since no plan changes how a container is actually started or stopped
(launcher-late-wake-failures changes only how the launcher's own start reports a failure, which the helm's tests cover
with stand-ins); the rest of the end-to-end suite, since the filter above covers every end-to-end test that stops,
restarts or relaunches a session or creates a host, which is where this round's changes act; the real-agent drift cases
themselves, which spend vendor turns and run only on request.
