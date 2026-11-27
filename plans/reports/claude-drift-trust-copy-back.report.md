### What this was about

The real-agent drift tests (the opt-in tests that run the actual Claude Code against Farhelm to catch changes in how it
behaves) run Claude with a private copy of its configuration. In that copy Farhelm removes the "trust this folder"
approval for the test's workspace and every folder above it, so a test cannot pass just because you once trusted a
parent folder. At the end of a test run, Farhelm copies some of the files Claude changed back into your real
configuration (a refreshed login, for example).

A review of the earlier change that strips trust raised a risk: if Claude saved that private copy during the run while
your real file stayed unchanged, the copy-back could write it over your real configuration and silently remove your
trust approvals for the parent folders. At planning, the code was checked and cannot do this (the trust files are always
private copies, never among the files copied back, and the copy-back separately refuses to overwrite them); you chose to
add only a regression test that keeps it that way, with no product changes.

What was built: exactly that. One test simulates a save of the private copy during a run (a new file renamed over the
old one, the way Farhelm itself saves the trust approval it adds) while your real file is left alone. Finishing the run
reports nothing and leaves the real file byte for byte as it was, with the parent folder's trust still set. The to-do
item that asked for this check is removed.

### Things you should know

- Nothing in the repository establishes how Claude itself saves that file, so the test models the save on how Farhelm's
  own trust write does it (write beside, then rename). Copy-back does not depend on how the file was saved, only on
  whether the trust file is among the ones it may return, so either way of saving is covered.
- Only one of the two existing protections is exercised here: that the trust files are never among those copied back.
  The other (copy-back refusing to overwrite a protected file) already had its own test.

### Open questions and possible follow-ups

None.

### The PRs

- #5134 `test: pin that a saved claude trust mirror never reaches the real config`: the test and the TODO entry's
  removal.

### Checks

Run on the pushed change (main has moved since only by queue bookkeeping):

- `cargo clippy -p farhelm --all-targets -- -D warnings`: clean.
- Through the test-run recorder (run 2ce3e58c-97b1-4828-9334-4c1aac05571f, 12 passed), the drift tests' private
  configuration tests, which run without Claude or any network:

  ```
  cargo nextest run -p farhelm --test e2e -E 'test(/real_agent_capture::drift::private_config::tests::/)'
  ```

- The test-sleep check (no unannotated delays), `cargo fmt`, and `dprint check` on TODO.md: all clean. The last edit
  after these runs only rewrapped a comment.

Skipped: the real-agent drift tests themselves, which are opt-in and spend real Claude turns; no product code changed.
The run above was narrowed from the plan's whole drift selection to these tests for that reason. No changelog entry,
since the change is test-only.

### Review gate

Reviewed by a fresh-context Opus 5.5 agent at high effort. It found no bugs and confirmed the test would fail if the
trust file were ever returned by copy-back, whether through the test's early check that the private copy really had the
parent's trust removed, or through the final byte comparison. Its findings were two comments that described the save and
the copy-back check inaccurately, a redundant import and a roundabout way of making that early check. All were fixed,
and a verification pass by the same reviewer confirmed them; its remaining wording suggestion was applied too.

### Landing

#### What landed with it

Since the executor last rebased this plan, main gained the previous rounds' changes to the app, the helm, the supervisor
and the command line. None of them touches the drift tests' private copy of an agent's configuration. In the same round,
three more plans landed: remove-legacy-sandbox-sessions-api (an unused helm route removed),
restart-startup-scope-cleanup (a supervisor start finishes a failed Restart's cleanup) and brick-test-inline-welcome
(release test instructions). None touches the drift tests.

An independent reviewer and I read the change and found nothing to fix.

#### Fixes made while landing

- **A TODO.md conflict.** The round's four plans each removed their own TODO entry, next to each other, so stacking them
  conflicted there. I removed all four entries; no code changed.

#### What the landing made untrue in the report

Nothing untrue, but the report undersells its test. It says only one of the two protections is exercised (that the trust
files are never among those copied back). The reviewer found that the test fails if either protection alone is lost:
without the first, the second (copy-back refusing to overwrite a protected file) turns the end of the run into an error,
which the test also catches.

#### Checks

Run once, on the round's four plans stacked on main as it stood when the landing started
(remove-legacy-sandbox-sessions-api, restart-startup-scope-cleanup, brick-test-inline-welcome, then
claude-drift-trust-copy-back), so each plan was tested together with the others and with the previous rounds' changes,
which none of the executors' own runs included. The SSH test worker was still unreachable, so the slow suites ran on
boat sandboxes (x86_64 Ubuntu 24.04 VMs), and clippy ran locally:

- Locally, `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`: clean.
- On three boats, every test of the protocol, helm, supervisor and command-line crates that runs by default, the
  end-to-end suite included (`cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor -p farhelm`,
  built once, split in three and run through the recorder), recorded runs 0aeec1d1-374d-4651-b817-c6196eba36e3,
  79bc70f6-81f9-4677-98ac-5bc0782c1540 and 0e083ecd-f092-4996-ae93-b1866a1bddfd: 4,732 passed, none failed, and no test
  that quietly passed for lack of systemd, Podman or SSH on the machine. That covers each executor's own selection and
  more: the helm's Tensorlake launch tests, the Restart cleanup tests, the startup tests of terminal hosts left over
  from earlier runs, and the drift tests' private-configuration tests.
- Locally, the portable brick-test harness
  (`python3 -B -m unittest discover -s releasing/mac-vm-test -p 'test_*.py' -v`), recorded run
  69a626cf-42a8-4fcc-8baa-85e5c53b0de6: 337 passed.
- Locally, `cargo fmt --all -- --check`, the test-sleep check (`scripts/check-test-sleeps.py`),
  `python3 releasing/check-changelog.py format`, `dprint check` on the changed Markdown, and
  `python3 scripts/check-agents-md.py` (the brick-test runbook is an instruction file): passed.

Reused: nothing. Skipped: the desktop app's and GUI crate's tests, the screenshot comparison and the end-to-end smoke,
since the only change to the app is two comments; the opt-in tests in those crates, among them the real-agent drift
tests, which spend real Claude turns; and a live brick test, which needs the Mac test machine and a release candidate.
