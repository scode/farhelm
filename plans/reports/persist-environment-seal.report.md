# Remember a sealed environment across helm restarts

## What this was about

When the user deletes the last session in a sandbox or container created for a session, its supervisor marks the
environment sealed for removal. If the removal failed and the helm restarted, the helm waited for the supervisor to
report the seal again. A suspended Tensorlake sandbox could sit empty in Hosts and continue incurring snapshot storage
charges.

The helm now remembers the seal before its first deletion attempt. After a restart it tries the deletion once from that
record, including a suspended sandbox whose supervisor it cannot reach. If the attempt fails, Hosts and the sidebar show
the reason and **Retry**.

## Things you should know

- The saved record holds the host and the time of the first attempt. The provider's current failure reason stays in
  memory. Confirmed removal also clears the seal, including when the provider finds the resource already gone.
- As decided during planning, a process inside a sandbox or container that Farhelm created for a session can cause that
  same environment to be marked for deletion. The helm can act on the saved mark after restarting.
- Startup can recover seals saved before a deletion attempt. A sandbox with an unsaved seal still needs a fresh
  supervisor listing or an explicit whole-sandbox Delete.

## Open questions and possible follow-ups

None.

## Pull requests

- Persist pending environment deletions, with coverage for startup deletion and Retry.
  [#4282](https://github.com/scode/farhelm/pull/4282/changes)

## Checks

- Ran `cargo fmt --all -- --check`, `cargo clippy --locked -p farhelm-helm --all-targets -- -D warnings`,
  `dprint check SPEC.md SPEC_impl.md TODO.md releasing/changelog.d/persist-environment-seal.md`,
  `python -B scripts/check-test-sleeps.py` and `python3 releasing/check-changelog.py format`. All passed.
- Reused the broad Linux nextest run from the working tree based on `d7d9d773` (recorder
  `054cd3bc-94c6-4726-ae88-99c6cda25d91`). All 357 selected tests passed. The selection covers the store, connection
  manager, Tensorlake host operations, environment launch and seal handling. The later diff tightened the startup test
  and clarified the specs.

  ```sh
  cargo nextest run -p farhelm-helm --lib -E 'test(/^store::tests::|^manager::tests::|^tensorlake::hosts::tests::|^provisioning::tensorlake_add::tests::/) | test(seal)'
  ```
- Ran the migration and restart cases on the final test source (recorder `163e9fab-cab5-4246-ab84-a5a12616589d`). All
  three passed, including the stronger check that startup clears the seal as well as removing the host.

  ```sh
  cargo nextest run -p farhelm-helm --lib -E 'test(remembered_seal) | test(schema_89_remembers_only_the_first_seal_attempt_until_host_removal)'
  ```
- The scripted Tensorlake API exercises the production deletion logic and observes that startup sends no supervisor
  connection. That covers the restart and Retry behavior without a live provider run. The wider workspace suite was
  skipped because the change is confined to the helm's store and environment lifecycle.

## Review gate

The required independent review found two omissions. The implementation spec now states the scope of the saved seal and
its authority to delete after restart. The startup test now waits for the seal to clear as well as for the host to
disappear. The review found no issue in the runtime logic.
