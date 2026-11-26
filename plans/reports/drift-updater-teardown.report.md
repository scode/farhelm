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
