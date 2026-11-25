# Host provisioning action fixes

## What this was about

A confirmed Uninstall on a Tailscale host used to resolve the destination again for individual removal commands. If the
address or keys behind the name changed, those commands could reach a different machine from the one Farhelm checked.
Uninstall now verifies the prepared peer at confirmation and keeps its address, keys and connection policy for the whole
run. It asks the user to reconnect or plan again when the session connection or Tailscale peer has changed.

Answering a shared SSH sign-in question could start an automatic update before the Update or Uninstall the user had
requested retried, leaving the manual action to wait for the host to become available. The agreed behavior gives the
explicit action first use of the answer. An automatic update can try again after the manual run settles or after 30
seconds without a manual run. Turning Tailscale on or off dismisses an automatic-only question and gives its connection
check a bounded time to finish. A question already owned by a manual action remains available to answer.

## Things you should know

The installer-identity check alone could accept an older Sessions connection and a fresh Uninstall probe if neither peer
reported an identity. Both confirmation session reads now check that the active Sessions connection opened with the same
Tailscale peer and policy as the removal commands. An unrelated change to a local SSH socket directory does not count as
a peer change.

If a manual request times out before Farhelm returns its sign-in prompt, the request may need to be retried. Answering
the shared question can still defer automatic Update for up to 30 seconds. That delay closes the gap between finding the
question and delivering the reply. When the user turns Tailscale on or off, Farhelm keeps the question open only if the
manual action already owns it. A cancelled or failed shared check suppresses further automatic prompts on that
connection.

## Open questions and possible follow-ups

Both planned changes fit the approved scope.

## Pull requests

- Uninstall keeps its confirmed Tailscale peer for the session checks and removal steps.
  [#4996](https://github.com/scode/farhelm/pull/4996/changes)
- Explicit host actions get priority after shared SSH sign-in, and mode edits retire automatic-only checks.
  [#5002](https://github.com/scode/farhelm/pull/5002/changes)

## Checks run, reused, and skipped

- The combined run on the rebased stack executed 91 selected cases successfully and recorded one runtime skip. It
  covered peer pinning and refusal, shared answer handoff in both join orders, retry after expiry or manual completion,
  failure across opt-out, and Tailscale route editing. Recorded run `784ef40d-b140-4732-9201-17095452751e`.
- Reproduce the selection with the pinned nextest through the test recorder. The repository's `.config/nextest.toml`
  sets four test slots and zero retries.

  ```sh
  python3 scripts/record-test-run.py --runner nextest --kind development \
    --selection "Rebased host provisioning stack and SSH paths" \
    --concurrency "4 nextest slots; retries 0" -- cargo nextest run -p farhelm-helm --lib \
    -E 'test(/automatic_sign_in::|automatic_update|automatic_admission|obsolete_route_refusal|tailscale_toggle|uninstall|ssh_sign_in::|ssh::tailscale::|access::tests::|transport::tests::|remote_routes_keep_existing_plan_json/)'
  ```

- `cargo fmt --all -- --check`, `dprint check`, `python3 releasing/check-changelog.py format`,
  `python -B scripts/check-test-sleeps.py` with the repository's isolated interpreter, and
  `git diff --check 607a91e8 0331d8d6` passed. Both PR diffs stayed intact through rebase. The combined run supplies the
  final runtime evidence. Earlier isolated peer-fixture development evidence remains in run
  `c8e99837-7507-41eb-a093-2c68b7b85fb3`.
- The live terminal-host test self-skipped because the workspace Farhelm binary was absent. The selected tests cover
  peer authority, shared question ownership, and route changes through isolated provisioning and SSH fixtures, so the
  wider workspace and UI suites were skipped. Real-host removal and a live terminal host stopping during removal remain
  unverified in this run.

## Review gate

Independent source review verified the session connection uses the same peer as the confirmed plan. The sign-in review
also checked Ready arriving before activation, failure after opt-out, and a queued reader returning after a mode change.
The final follow-ups found no remaining issues.
