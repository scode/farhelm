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
