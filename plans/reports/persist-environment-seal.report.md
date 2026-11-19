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

### Landing

#### What landed with it

It landed second in its round, after docs-shot-desktop-fixes (desktop wording, a session-row id and template summary
ellipses); the two only met in TODO.md, where each removed its own entry. Since its base, main gained
emacs-text-field-keys and a plan for SSH sign-in prompts, neither of which touches the helm's hosts or environments.
Main gained no helm database version meanwhile, so the new one (89) needed no renumbering.

At your request this landing was reviewed with extra skepticism, by me and by an independent reviewer, looking for
over-building, weak code or documentation, and anything a user sees that makes no sense. The reviewer checked:

- The database step: the version constant, the new-database schema and the migration agree, and existing hosts get no
  remembered deletion they never had.
- The ordering: the deletion is remembered under the host's provisioning lock, before the provider is asked to destroy
  anything, and remembered deletions load before anything is published.
- That removing a host clears what was remembered for it.

#### Fixes made while landing

- **A database test fixture missed the new column.** The test that rebuilds a helm database as it was at version 26
  drops every host column added since, then compares the result with the frozen version-26 schema. It did not drop the
  new `environment_sealed_at` column, so two checkout-settings tests failed. The plan's checks had not run them; the
  landing's full Rust run did. The fixture now drops it, newest first, as it does the other environment columns.
- **"Tries once" was loose** in SPEC.md and SPEC_impl.md. After a restart the helm makes one attempt at each remembered
  deletion as soon as it can take the host. While another operation holds the host, it retries every 10 seconds, and
  only a failed attempt waits for Retry. Both specs now say so.

#### Checks

The round changes the desktop app's GUI crate (with republished reference screenshots), a patch to the vendored GPUI
copy, a docs-shot page module, and the helm's store, hosts and sandbox paths. So the landing ran the whole Rust
workspace suite, the app's end-to-end smoke, the vendored GPUI tests, all three clippy runs and the reference
screenshots (goldens). This round was the first to run the slow checks on the maintainer's own test machine instead of
boat sandboxes. It is an arm64 Linux VM that builds and tests the stacked tree itself, so those runs are arm64 evidence.
The goldens, formatting and the small checks ran locally on x86_64.

On the two plans as delivered, stacked on main with their TODO.md edits kept side by side:

- `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py`, `dprint check`,
  `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` and
  `python3 -B scripts/test-docs-shots.py`, locally: passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and
  `cargo clippy --locked -p farhelm-gui --all-targets --features agent -- -D warnings`, on the test machine: passed.
- `python3 scripts/check-gui-goldens.py`, locally: passed with the republished references
  (`588e6641-c358-49e4-944b-93023fc2be28`).
- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test machine
  (`97b14dc5-9010-411d-89ee-5f546ff09a81`): 8097 of 8101 passed. The two checkout-settings fixture tests failed because
  of the missing column fixed above. The other two failures were setup gaps on the new machine, not code findings: the
  macro compile-contract test found no default Rust toolchain outside the repository, and the supervisor's
  window-command test read Ubuntu's sudo hint from a login shell. Twelve OMP tests printed SKIPPED because Bun was not
  installed. All three gaps are fixed in the machine's setup and covered by the reruns below.
- The end-to-end smoke (`python3 -B scripts/gui-e2e-smoke.py`), on the test machine: 49 of its 50 steps passed, and the
  clipboard paste step was skipped, as always on Linux (`5ac8ce26-0f1c-45e8-9751-46a39eb3a35f`).
- The vendored GPUI tests (`cargo test --locked --manifest-path vendor/gpui-pre/Cargo.toml --features test-support` with
  `--test native_key_metadata`, `--test driver_record` and `--test hitbox_occlusion`), on the test machine: all 5 passed
  (`a1ac0f9a-4cd8-483d-ae74-6ab1f82590c5`).

On the fixed tree, on the test machine:

- The helm's whole test suite, every OMP test, the macro compile contract, the window-command tests and the Templates
  dialog tests:
  `cargo nextest run --workspace --exclude farhelm-desktop -E 'package(farhelm-helm) | test(/window_command_passes_hostile_paths/) | package(farhelm-testtrace-macros) | test(/omp/) | test(/^templates::dialog_tests::/)'`
  (`2100c8a1-af81-4db6-84ed-9833439a8583`): 2212 of 2214 passed, with no runtime skips. The two failures were the
  window-command test, whose sudo hint the machine's setup had not yet silenced, and a Templates check I had tried to
  restore and then took out again (see the first plan's notes).
- The window-command and Templates dialog tests again, after both fixes (`c4a720b1-e7d8-4770-bc30-06beee657be0`): all 37
  passed.
- `cargo fmt --all -- --check` and `dprint check` on SPEC.md, SPEC_impl.md and `vendor/README.md`, locally: passed.

The rest of the workspace suite, the smoke, the vendored GPUI tests, clippy and the goldens were reused for the fixed
tree. After them only a test fixture's SQL, two spec sentences and a README changed.

Skipped:

- Anything on a Mac or on x86_64 Linux for the runtime suites: they ran on arm64 only this round. The release gate still
  runs them on x86_64.
- A real Tensorlake or Podman deletion across a helm restart: the plan's tests use stand-ins for the providers.
