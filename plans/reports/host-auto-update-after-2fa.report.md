## What this was about

Automatic updates could stall on a remote SSH host that asked for a password or verification code. Farhelm uses separate
SSH connections for sessions and setup. Signing in for sessions could get the host connected while the update still
needed its own sign-in. The automatic update gave up on that connection and left the Update badge for the user to act
on.

The plan called for automatic updates to resume after setup sign-in and for the question to appear quietly in the
desktop attention row. An eligible host now offers **Answer** there with the usual attention cue and sign-in sound. The
form opens when you choose **Answer**, so typing in a terminal can continue. Once SSH accepts the answer, the host's
update can resume even while another host is slow to respond.

## Things you should know

- A cancelled, expired, or refused setup sign-in leaves the Update badge. Routine checks wait for a fresh connection
  before trying again. Signing in again or choosing **Retry** on a host with an incompatible supervisor now also gives
  automatic updating a fresh chance.
- When an automatic update and a manual **Update** or **Uninstall** both need setup sign-in for the same host, you see
  one question. Both actions can use that sign-in. Farhelm allows one provisioning operation at a time on the host.
- Turning off automatic updates dismisses a question opened solely for the automatic check. Turning the preference back
  on lets that check ask again. A question shared with a manual action stays available for that action.
- The desktop already had the quiet attention-row behavior. The plan exercises it in an untouched window and immediately
  after a Sessions sign-in on the same host.

## Open questions and possible follow-ups

If the setup connection expires after you answer and before the automatic update can use it, that update can fail and
leave the Update badge. You can choose **Update** to sign in again.

## Pull requests

- Retry automatic updates after reconnecting to a host whose supervisor is incompatible.
  [#4721](https://github.com/scode/farhelm/pull/4721/changes)
- Resume automatic updates after setup sign-in. [#4726](https://github.com/scode/farhelm/pull/4726/changes)

## Checks

- Ran recorded nextest on the rebased stack with
  `cargo nextest run -p farhelm-helm -p farhelm-gui --lib -E 'test(/^provisioning::tests::.*automatic/) | test(/^provisioning::tests::ssh_/) | test(provisioning::service::automatic::tests::) | test(staged_payloads_require_manual_updates) | test(/^preferences::tests::/) | test(/^sign_in::tests::/)'`.
  All 47 selected tests passed on `7a3ea7ea`, run `56396fa5-b485-48d3-8e4d-daa58d83f66d`. That includes the final
  eligibility check and desktop focus after the button and update reminder changes on main. The draft tip has the same
  tree as that tested revision.
- Passed `cargo fmt --all -- --check`, `dprint check SPEC.md SPEC_impl.md TODO.md`,
  `python3 releasing/check-changelog.py format`, `git diff --check 53e5b510 7a3ea7ea`,
  `python -B scripts/check-test-sleeps.py` using the isolated interpreter,
  `cargo clippy --locked --all-targets -- -D warnings`, and `cargo clippy --locked -p farhelm --bins -- -D warnings` on
  the rebased stack.
- Reused unchanged cases from
  `cargo nextest run -p farhelm-helm -p farhelm-gui --lib -E 'test(/^provisioning::/) | test(/^manager::tests::/) | test(/^preferences::tests::/) | test(/^sign_in::tests::/)'`,
  550 passed on `32cbabf9` in run `19acb12e-9741-4944-b26c-1814a1c07a94`. Also reused
  `cargo nextest run -p farhelm-helm --lib -E 'test(/^manager::tests::/)'`, 114 passed on `cb4ceb78` in run
  `bc16d6b1-d169-4053-b655-39b70baa1c3a`. The manager sources and fixtures it exercises kept the same contents through
  the rebase. The new eligibility test and provisioning and desktop interactions affected by the rebase were tested on
  the rebased stack.
- An existing Tensorlake test for carrying the Codex sign-in choice into environment setup failed in run
  `c93ce2cd-335f-4db5-8b35-f7873bc9e3e9`. It passed an exact replay in run `d24e7470-e4bf-4b3a-94be-c5403915fa40` and
  the subsequent 550-test run. The cause remains unverified.
- Skipped the full suite and visual captures. The focused and retained tests cover the changed behavior, and the desktop
  change adds tests for the existing presentation. SSH sign-in was exercised locally with a simulated remote host and
  Farhelm's answer handling.

## Review gate

The independent Sol reviewer checked both steps. Findings led to keeping one sign-in question when manual and automatic
requests race, dismissing automatic-only questions as soon as the preference is turned off, and rechecking that the host
connection is still eligible before starting an update. The last review confirmed the remaining findings were resolved.
The commit wording also passed cold review.

### Landing

#### What landed with it

The plan's two PRs (#4721, #4726) and one landing fix (#4730) landed alone in their round. The executor had already
rebased them onto a main that held the three plans landed just before (removing a host during sign-in, the desktop
button restyle and the pending-update pill), so main had gained nothing but plan bookkeeping since the stack's base and
the stack applied without conflicts.

The landing's independent reviewer read the plan against what the remove-host plan put on main, since both change the
setup connection's sign-in checks. Removing a host also cancels a sign-in check an automatic update opened, and a
sign-in that succeeds while the host is being removed starts no update. The reviewer also confirmed that a manual action
and the automatic update waiting on the same host share one question, and that turning automatic updates off cancels
only the questions opened for them. The plan gives a host with an incompatible supervisor a fresh automatic attempt when
the helm newly sees it as incompatible or when you act on its connection (Retry, editing its address, taking it over and
the like), and never on the periodic re-check of a host that stays incompatible, so a failed automatic update cannot
retry in a loop. Nothing else in the helm or the desktop that tracks a host's connections reacts to this, since it all
acts only on a connected host. There is no wire protocol or database change.

#### Fixes made while landing

- #4730 rewords one SPEC.md sentence. It said an automatic Update "never opens the Setup question by itself", which,
  next to the Topology section saying the update does ask through **Answer**, read as if automatic updates never asked.
  It now says the question waits in the attention row and its popover opens only when you press **Answer**. The same PR
  corrects a test comment that misnamed the event the test sends.

#### Checks

The plan changes the helm's host connection tracking, automatic Update and setup sign-in, and adds desktop sign-in
tests. Its own runs were filtered selections of the helm and desktop window libraries, while the change alters when a
host counts as reconnected, which the end-to-end suite's helm and the desktop's host views read. So the landing ran the
whole workspace suite, the lints and the end-to-end smoke (its host Update, setup and sign-in steps against fake SSH
hosts) on the plan stacked on main. The worker runs were on a separate arm64 Linux test worker; the rest ran locally on
x86_64 Linux.

On the plan stacked on main, before the landing fix:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `e373ca13-f168-422f-9209-23424a675d5d`): all 8483 passed and none printed a runtime `SKIPPED`. That includes the
  Tensorlake test the report says failed once during execution; this pass says nothing new about why it failed then.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `de5ad05e-224e-4abe-bc54-3089dd86303f`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which the headless Linux desktop session the
  smoke runs in cannot provide).
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix, which changes one SPEC.md sentence and one test comment: `cargo fmt --all -- --check` and
`dprint check SPEC.md`, locally, passed; the runs above are reused, since the fix changes no code.

Skipped: the desktop window's reference screenshot comparisons, since no drawing code changed; the tests and lints of
the desktop app's own startup and packaging code, since the plan changes no interface that code uses and it does not act
on when a host reconnected; the workspace doctests, since the plan's doc-comment edits add no examples; macOS; CI, which
runs only on demand.

#### Things you should know that the report does not say

- **An automatic update can get ahead of an Uninstall you asked for.** On an outdated host that needs a verification
  code, if you choose **Uninstall** (or **Update**) while the automatic update's question is waiting, both share that
  one question. Once you answer, the automatic update starts first, and your confirmation then fails because the host is
  busy; choosing it again after the update finishes works. This is new: before the plan, the automatic update had
  already given up on such a host, so it never competed with you. A possible follow-up is to have the automatic update
  step aside once a manual action has joined its question.
- **A waiting automatic question blocks that host's Tailscale toggle.** While the question waits, changing the host's
  Tailscale mode is refused as busy, for up to the question's two-minute expiry. Manual Update and Uninstall questions
  already did this, but there you had started the action yourself.
- **An outdated host that needs a verification code asks again on every reconnect.** A question nobody answers is
  remembered only for that connection, so while automatic updates stay on, each reconnect of such a host brings a new
  question with its chime; a laptop that sleeps and wakes often would see one each time until someone answers or turns
  automatic updates off. This is what the spec and the plan's decision ask for, but it is a new recurring prompt worth
  watching in use.
