# Report: Stop refuses a session whose start was never confirmed

## What this was about

Sometimes a session's start is never confirmed. That happens when a create was interrupted (the helm or supervisor went
away partway through), or when the host reported an error for a create even though the agent might still start. Such a
session is listed with an unknown status and has no main terminal yet, while its agent may still be starting.

Before this change, pressing **Stop** on such a session behaved two ways:

- From the session's ended-agent display, the GUI asks Stop to proceed only if the agent has ended (a guarded Stop).
  That Stop already refused, with a message telling you to check the session and try again.
- Any other Stop (the session menu on a row with unknown status, the command line, the REST API, an agent asking to stop
  its session) recorded an exit that nobody had seen and reported success. The session then showed as stopped while its
  agent could still finish starting, so an agent could then be running with nothing in Farhelm showing it.

The TODO entry asked for Stop never to report that false success. When the plan was written, you decided Stop should
refuse rather than gain a pending-stop mechanism. Your reasons: Delete already settles such a start (records that the
start is over and ends whatever it left running), and a session that never started has no main terminal to keep.

What is on main now:

- Every Stop of such a session is refused, whoever asks, and nothing changes. The message says the start was never
  confirmed, so there is no terminal for Stop to stop and nothing was stopped, and that the agent may still be starting.
  It tells you to Delete the session, or to Restart it if Restart is offered.
- Delete then ends whatever the start left behind, as it did before.
- SPEC.md's Stop rule and the implementation notes say the same, and the TODO entry is removed.
- A changelog entry describes the fix.

## Things you should know

- **Restart is named only "if it is offered".** The plan suggested pointing to Delete or Restart. The review pointed out
  that Restart is usually not offered here. Restart resumes a captured conversation, and a create that was interrupted
  before its agent ran has none (and Bash or Zsh sessions never have one). So the message, SPEC.md and the changelog
  entry name Delete as the way out, and Restart only where it is offered.
- **One more case falls under the refusal.** A Restart that fails without knowing whether its new agent started leaves
  the session in the same state: start unconfirmed, no main terminal.
  - Before this change, an ordinary Stop there did end the new launch's processes and reported success. It left
    Farhelm's record of the unsettled start in place (the record Delete closes when it marks a start as over), which a
    later Delete or Restart closed; I know of nothing a user saw from that.
  - Now Stop refuses there. To stop that agent and keep the session, Restart it again (it has a conversation, so Restart
    is offered) and then Stop. Delete also works, but removes the session, so it can no longer be restarted into that
    conversation from Farhelm.
  - So for this one case the change takes something away: a Stop that used to work honestly is now refused. The plan's
    reasoning, that a session that never started has no main terminal to keep, does not fit it, since this session has a
    conversation and history. I kept the refusal uniform, as the plan asked ("whoever asks"), wrote this case into
    SPEC.md's Stop rule, and raise it below.
- **Stop and suspend is refused for such a session too**, because it runs this same Stop first. Before, an ordinary Stop
  and suspend went on to suspend the disposable environment after Stop's false success.
- No stored data changes, and nothing about the GUI's own Stop confirmation changes.

## Open questions and possible follow-ups

- After a Restart that failed without knowing whether its agent started, should Stop go back to ending the new launch's
  processes instead of refusing? The options:
  - Keep the refusal (what is on main). One simple rule: Stop never acts on a start it cannot see. The cost is an extra
    step in a rare failure: Restart and then Stop, or Delete if the session is no longer wanted.
  - Let Stop act in this one case. Stop would end the new launch's processes, as before, and also close the start's
    record the way Delete does. That brings back the plain Stop, at the price of a second kind of unconfirmed start that
    Stop treats differently from an interrupted create.

  I recommend keeping the refusal: the failure is rare, the way out keeps the session, and the rule stays one sentence.

## PRs

- #5091 fix: refuse Stop on an unconfirmed start (the refusal, SPEC.md and implementation-notes text, tests, the TODO
  removal, the changelog entry).

## Checks

Run now, on the change as it will land (after the review fixes and the rebase onto current main):

- Through the test recorder, run `c2f64ba2`, 267 passed. The selection, in the supervisor and the command-line
  end-to-end suite: every test with "stop" in its name, every test of sessions without a terminal, the supervisor's
  session-control unit tests, and the tests that launch agents in a session's terminal. None reported skipping for a
  missing systemd or SSH substrate. That includes the new end-to-end test: Stop refuses a create that was interrupted
  while its launch was paused, leaves that paused launch and the start's record untouched, then Delete ends the paused
  launch and the agent never runs.
- `cargo clippy -p farhelm-supervisor -p farhelm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, the test
  sleep check, `dprint check` on SPEC.md, SPEC_impl.md and TODO.md, and the changelog format check.

Earlier runs, superseded by `c2f64ba2`: run `9f97ee17` (the same selection without the session-control unit tests: 179
passed, 1 failed, the old unit test for the guarded Stop's refusal, which expected the old message; I rewrote it to
cover the guarded and the ordinary Stop), run `de7923b1` (the supervisor's own tests after that rewrite, 168 passed) and
run `052cf011` (the session-control unit tests and the create tests after the review fixes, 122 passed).

Reused: none.

Skipped: the full end-to-end suite and the desktop app's tests. The change is one refusal in the supervisor's Stop, and
run `c2f64ba2` covers every test that exercises Stop or this kind of session.

## Review gate

A fresh-context Opus 5.5 review at high effort found no blocker. It traced every path into Stop (desktop app, command
line, REST API, agent requests, Stop and suspend) and confirmed that the refusal fires only for this state and before
anything is recorded or signalled. It also confirmed that Delete and Restart settle the state.

It raised two should-fix findings: the Restart wording and the failed-Restart case described above. Both are addressed
as described. It also raised five small points: an implementation-notes sentence that called agent-approved Stop
unrestricted, a test comment's wording, an explicit "no host" premise in the new test, a redundant assertion, and a
constant visible more widely than needed. All were fixed. The fixes were not reviewed again; run `c2f64ba2` ran after
them.

### Landing

#### What landed with it

This round landed session-right-click-menu and launcher-late-wake-failures before this plan, and drift-updater-teardown
after it; see the first plan's notes for the round. The four plans' changes overlapped only in TODO.md. The change on
main that matters here reached main after this plan was built: Stop and Restart now end an agent that outlived its
killed terminal host (#5083), in the same part of Stop this plan edits. The rebase applied cleanly, but a clean rebase
can still combine two changes badly, so two independent reviewers and I checked them together. They cannot both apply to
one session: this plan's refusal covers a session with no recorded main terminal, and #5083's cleanup of a leftover
agent only runs for a session that has one. That cleanup is still in place in Stop and in Restart, untouched, and a
session with no recorded terminal never reached it before this plan either. Stop refuses before asking for any
confirmation and leaves the session exactly as it was, as SPEC.md requires of a refusal. The new right-click menu for
marked sessions (session-right-click-menu) sends ordinary Stops, so such a session gets the same refusal on its row.

#### Fixes made while landing

None for this plan. The reviewers found nothing to fix.

#### Worth knowing

- **In two of the cases the refusal covers, the old Stop actually worked.** One is the Restart that failed without
  knowing whether its new agent started, which the report covers. The other is a create where the host reported an error
  after the agent's terminal was already running: Farhelm never recorded that terminal as the session's main terminal,
  but it existed, and the old Stop ended it. In both cases the refusal message ("there is no terminal for Stop to stop")
  is not quite right, and Stop can no longer close the session's terminal tabs either; the report does not mention tabs.
  The way out after a failed Restart is Restart and then Stop. After a failed create, Restart is usually not offered, so
  the only way out is Delete, which removes the session.
- This bears on the report's open question. The reviewers and I agree with its recommendation to keep the refusal, but
  the tabs point may change your view. Possible follow-up either way: reword the refusal message for these two cases.

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
  covers the re-render launcher-late-wake-failures' report skipped after its rebase.
- Locally, `python3 -B scripts/test-gui-screenshot-batch.py`, which checks the screenshot scene list against the golden
  manifest: recorded run b81cb326-fd3c-4921-9f49-69ff01bb3f0d failed because the new late wake failure picture was
  missing from the scene list; after the landing fix in launcher-late-wake-failures, recorded run
  ffecfaab-79e4-4bfd-942e-a25925a47bd5 passed.
- Locally, the drift runner's own tests (`python3 -B scripts/test-run-harness-drift.py`), recorded run
  029c1d18-8368-4a93-b71c-ea765289da54: passed, on the landing machine's systemd user manager (Linux).
- Locally, `python3 scripts/check-agents-md.py` (0 errors; its one warning is an old one about the frozen triage
  archive), `cargo fmt --all -- --check`, the test-sleep check, `python3 releasing/check-changelog.py format` and
  `dprint check` on the changed Markdown: passed.

Reused: nothing. Skipped: the Podman suites, since no plan changes how a container is actually started or stopped
(launcher-late-wake-failures changes only how the launcher's own start reports a failure, which the helm's tests cover
with stand-ins); the rest of the end-to-end suite, since the filter above covers every end-to-end test that creates,
stops or restarts a session or host; the real-agent drift cases themselves, which spend vendor turns and run only on
request.
