## What this was about

When Farhelm could not read a saved Claude or Codex recipe credential, an update could erase that agent's entry from the
shared launch credentials file on a connected host. Even saving an unrelated GitHub token could trigger the loss.

The plan called for keeping both agents' last delivered launch credentials and naming the failed lookup in the host
warning. The shared file and its delivery records stay intact. Farhelm can still deliver a separate Codex subscription
login or GitHub token.

## Things you should know

- Host details name each failed lookup and include any later delivery error. Saving either agent's shared launch
  credential reports the failed lookup because the replacement could not reach the host. A Claude lookup error during
  host setup is also reported.
- A separate delivery error can still appear in a save reply after an earlier file reached the host. For example, saving
  a Codex subscription login can install the new login and still report a later GitHub lookup error. If the Claude
  launch lookup also failed, host details carry that reason and the subscription login's save reply reports only the
  GitHub error. If the separate Codex login lookup fails while the launch file is held, Farhelm can still deliver a
  valid GitHub token.
- When Farhelm cannot remove an old managed credential from an agent's settings file, it still leaves that agent's new
  launch entry out until cleanup succeeds.

## Open questions and possible follow-ups

No open follow-ups from this plan.

## Pull requests

- Preserve recipe launch credentials when a lookup fails. [#4751](https://github.com/scode/farhelm/pull/4751/changes)

## Checks

- Ran the recorder with `cargo nextest run -p farhelm-helm --lib -E 'test(credential_delivery::tests::)'`. All 33
  selected tests passed on `e9aa4fe2` (run `5e0ac01a-d6ad-4787-bb41-86f1f5553786`), with no runtime skips. The selection
  covers both saved launch credentials, setup, separate login and GitHub delivery, warning routing, existing cleanup
  behavior, and delivery on active connections.
- Ran `cargo clippy --locked -p farhelm-helm --all-targets -- -D warnings` on `e9aa4fe2`. It passed. Reused that check
  and the 33-test pass after rebasing over #4747, which changed the desktop update pill. The helm code, fixtures and
  dependencies tested here kept their contents.
- Passed `cargo fmt --all -- --check`,
  `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/codex-recipe-launch-credentials.md`,
  `python3 releasing/check-changelog.py format`, `git diff --check 5b1cfcdb 21099ef2`, and
  `python -B scripts/check-test-sleeps.py` using the isolated interpreter on the rebased working revision `21099ef2`.
  The draft has the same tree.
- The earlier nextest selections passed 32 tests each at `326a6e12` (run `f92292f8-ac44-49b4-8bc6-3cc84afaf3d5`) and
  `b3dd33e6` (run `cb06b6dd-1ab7-4c54-b925-2e72eb7ffe6c`). Repeated the selection after the save reply changed.
- Skipped the full workspace suite and desktop captures. The helm selection exercises the changed delivery and warning
  contracts, and the desktop uses the existing warning presentation.

## Review gate

The independent Sol review found that a later delivery error could hide the original lookup warning. The follow-up
review found that sending the combined warning to every save reply would give an unrelated login save a misleading
lookup reason. A scope review confirmed that host details can show both reasons while saves keep any existing delivery
error and add only lookup warnings for the credentials they use. The final Sol re-review found both issues resolved. An
independent reader checked the commit wording.
