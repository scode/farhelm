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
