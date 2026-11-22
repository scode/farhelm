## What this was about

Two SSH entries could show the same Podman store as two container hosts. If an SSH alias started reaching a different
machine or account, Delete could see an empty store and forget a container that was still running at the old address.

For each machine and account, the launcher and Settings offer one entry for starting containers and building images. The
container-host check names the winning entry when it finds a duplicate. Usually the first entry checked wins. Farhelm
can prefer the entry whose supervisor it has verified when the other entry has no containers or recipe artifacts to keep
track of. Entries with resources can still be used to clean them up. After cleanup, a refreshed check can accept a
changed destination. The same machine and account checks work on a host without Farhelm installed.

## Things you should know

Farhelm records where each new container runs and checks the machine and account when it creates, checks or deletes the
container. If the destination changes, Delete leaves the container and its sessions listed. Restoring the original SSH
destination lets you try Delete again. A sealed container shows the failure with Retry in Hosts. Recipe cleanup keeps
images and helper records when the saved machine and account cannot be verified. Rebuild keeps the previous pair of
images until both replacements are ready.

As decided during planning, containers already created before this change retain the old Delete behavior. If their SSH
destination changes to an empty store, Delete can still remove the entry while the old container keeps running. New
container work requires the Linux commands `sha256sum` and `tr`. The launcher and Settings show what is missing if
either command is unavailable. The original machine ID stays in the helm's database. The destination receives a digest
for that command only.

## Open questions and possible follow-ups

None.

## Pull requests

- Verify the host's machine and account before new container work.
  [#4632](https://github.com/scode/farhelm/pull/4632/changes)
- Keep new container entries when the SSH destination changes.
  [#4635](https://github.com/scode/farhelm/pull/4635/changes)

## Checks run, reused and skipped

- Run on final revision `7f2286d5`. `cargo clippy --locked -p farhelm-helm --all-targets -- -D warnings` passed. The
  real local rootless Podman acceptance built and used the same-revision payload and completed container creation,
  lifecycle and owned image cleanup. Run `95301123-2d8c-4386-8e5d-a4431d9c81ef`.
  `cargo nextest run --locked -p farhelm --test e2e -E 'test(=podman_environments::real_local_environment_lifecycle_and_owned_image_cleanup)'`
  Rustfmt on the test-only correction and final diff whitespace passed.
- Reused from `83fb0864`. The focused helm selection passed 65 tests, including same-shell refusal, Delete and Retry,
  Rebuild fallback, and fresh versus migrated schema parity. Run `1dedadf0-363a-438b-abdd-98a0c896dab4`. The final
  change only rewrote an equivalent byte literal in the command stand-in for Clippy. Rustfmt, changed Markdown
  formatting, changelog format, and the test-delay checker also passed at that revision. The focused selection was

  ```sh
  cargo nextest run --locked -p farhelm-helm --lib -E     'test(/^environment::podman/) | test(/^store::podman::tests::/) | test(/^access::tests::container_access/) | test(a_container_check_reaches_the_connected_ui_when_it_starts_and_when_it_ends) | test(a_migrated_database_matches_a_freshly_created_one) | test(update_ssh_destination) | test(set_destination)'
  ```

- Skipped the opt-in real SSH lifecycle because the plan calls for the local real-engine check. The helm tests cover
  moved SSH destinations using scripted transports. Skipped GUI captures because the changes reuse the existing display
  and actions, and the broader workspace battery because the selected tests exercise the affected paths.

## Review gate

The independent Sol review and final verification cleared both PRs. The final review confirmed the rebased stack keeps
the landed Rebuild publication order. Both PR messages passed a cold read.

### Landing

#### What landed with it

The plan's two PRs (#4632, #4635) landed alone in this round, followed by one landing fix (#4636). Since the plan was
written, main had gained the change that lets independent desktop actions, among them environment Delete and Suspend, be
answered out of order (#4631); nothing in this plan relies on requests arriving in order, and the rebase applied
cleanly.

An independent reviewer found no way for the plan to make Farhelm forget a live container that has a recorded machine
and account, and no way for two entries for the same machine and account to both accept new containers. It found one
wrong error message and two missing release-note caveats, which the landing fixed; the landing's own test run found the
third problem below.

#### Fixes made while landing

- The workspace test run failed six of the helm's database tests. They turn a fresh database back into an older version,
  upgrade it, and compare the result with a fresh one, and they had not been taught to remove what the plan's two new
  upgrade steps add. The report's "fresh versus migrated schema" check is a different test, which passed here too; the
  executor's focused selection did not include these six. #4636 fixes the tests; the upgrade itself was right.
- Before each Podman command, the plan has the container host prove it is the recorded machine and account by printing a
  line only the right host can produce. A host that was asleep or offline, where SSH never connected and so nothing was
  printed, was reported as "its SSH destination may have changed", and the real SSH error was dropped. #4636 keeps that
  message for the check's own refusal and otherwise says the host could not be reached, with the SSH error text. Either
  way the command still fails and its output is never read as Podman's answer (such as "no such container"), so no
  container can be forgotten through it.
- #4636 adds two caveats to the plan's changelog draft: creating containers and building their images now also need
  `sha256sum` and `tr` on the container host, and if two existing host entries reach the same machine and account, only
  one keeps accepting new containers.

#### Checks

The plan changes how the helm records and checks each container host's machine and account, with two database steps, so
the landing ran the Rust workspace suite (including the real Podman environment test) and the lint runs on the plan as
delivered, then the helm's own suite again after the landing fix. No desktop code changed, so the desktop app's tests,
the end-to-end smoke test and the reference screenshots were not run. The worker runs were on a separate arm64 Linux
test worker with a systemd user manager, rootless Podman and SSH to itself; the rest ran locally on x86_64 Linux.

On the plan as delivered, rebased onto main:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `5d4f515e-f43b-4fcd-837e-8c4f73d7bd63`): 8368 of 8375 passed, none printed a runtime `SKIPPED`, and the real Podman
  environment test ran and passed. Six of the failures were the helm database tests the landing fix corrects. The
  seventh, an end-to-end test of two concurrent session creates, timed out once waiting for its first create; it does
  not touch container hosts, and it then passed 20 times in a row on the same worker, each attempt through the recorder;
  the 20 runs' ids stayed on the worker, and the worker session that ran them is `d96e4a50-c680-4583-8323-51b5add0fbc2`.
  It is not a known flake, and one timeout is not enough to record it as one; the failed run is kept.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fix (#4636):

- `cargo nextest run -p farhelm-helm --lib`, on the test worker (recorder run `87f871e5-84a6-4036-b171-939a47117561`):
  all 1833 passed, among them the six database tests, the container host check's tests and the rest of the container
  host and environment tests.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed, also after a last wording change to comments and the
  changelog draft.

Reused: the workspace run above for everything outside the helm library, since the fix changes only the helm's container
host check, its tests and the changelog draft; the real Podman environment test passed there and the fix changes only
the wording of an error it does not reach. Skipped: macOS, since nothing here builds only on a Mac; CI, which runs only
on demand.

#### Things you should know that the report does not say

- The launcher and Settings name a missing `sha256sum` or `tr`, as the report says, but if a command runs into the check
  failing for that reason, or because the host cannot read its machine ID, it still shows "destination may have
  changed". A small follow-up could tell those apart; nothing is lost meanwhile.
- The reviewer questioned whether hashing the machine ID for each command is worth the extra `sha256sum` and `tr`
  requirement, against comparing it directly, and judged it defensible since the recorded ID is then never sent to the
  host. No change made.
- If a host's login shell prints something before the proof line, a command can run but its reply still counts as
  unproven, and the user sees "could not reach this container host (exit status 0)". It fails safe, and the old wording
  was no better; nothing to do unless it is seen in practice.
