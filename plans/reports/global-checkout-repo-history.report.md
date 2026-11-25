# Managed checkout repository history

## What this was about

In New session, Managed checkout could ask an existing host to search its checkout folder. A new sandbox could show
repositories from Local even though it could not use those clones. An offline or unconfigured host could also make
suggestions incomplete. You chose one launch history across the helm and asked to carry past launches forward on
upgrade.

## Things you should know

Repository and `gh:` search offer the same history of up to 100 recently used Managed checkout repositories across
destinations, including new sandboxes, containers and offline hosts. You can still enter a new repository as
`owner/repo`. Successful launches bring the repository back to the top, including when you replace a session on an
existing disposable host.

Upgrades bring over repositories still recorded in ordinary and disposable launch history. If you removed an ordinary
host before the upgrade and its launch records are already gone, the upgrade cannot recover those entries. Retained
disposable-environment history can still supply entries after the environment has been removed. Once upgraded, removing
either kind of destination preserves its repositories in the shared list.

An existing host that needs a checkout folder still offers setup in the launcher. After Save, the launcher updates the
checkout location for that host. If you switch hosts while the save is finishing, the launcher uses the destination
host's folder. If Launch wakes a suspended host, the original click continues the launch once that host is ready.

The helm and existing host supervisors need compatible versions after the update. A supervisor still on the old version
cannot connect to the upgraded helm until the host is updated. The Start a session guide explains the shared history.

## Open questions and possible follow-ups

During release testing, bring an existing host with an older supervisor across the normal update path. Confirm it
reconnects after the host update and that you can select a recent repository and launch a Managed checkout there.

## Pull requests

- Store the shared launch history and carry retained repositories forward on upgrade.
  [#4928](https://github.com/scode/farhelm/pull/4928/changes)
- Use the shared history in the launcher, keep checkout-folder setup, and remove host scanning.
  [#4938](https://github.com/scode/farhelm/pull/4938/changes)

## Checks

- Store checks passed at `d48d9b948361`. The rebased draft has the same patch and the intervening changes preserved
  store, migration and replacement admission behavior. The recorder ran these two selections with the pinned nextest,
  four slots and zero retries. Six focused tests passed in `d93dc6b8-383e-471e-95d5-12b2c613416b` and 297 wider tests
  passed in `511fd0df-3f70-4eec-b949-e54d9c7ad989`. Helm all-target Clippy also passed.

  ```sh
  cargo nextest run -p farhelm-helm --lib -E 'test(global_checkout_repository_history) | test(checkout_migration_from_26_recreates_the_config_tables) | test(checkout_open_existing_refuses_older_schema_without_migrating) | test(fresh_replace_on_a_session_owned_host_uses_its_home_without_setup) | test(replacement_checkout_history_has_its_own_admission_window)'
  cargo nextest run -p farhelm-helm --lib -E 'test(/store::/) | test(github_repo) | test(checkout) | test(migration) | test(captured_create_history::) | test(fresh_replace_on_a_session_owned_host_uses_its_home_without_setup)'
  ```

- Launcher checks passed at `547b9944a5a5`. The published draft differs only in one test fixture, where two guards that
  returned the same response were combined. Three focused tests passed in `60f3845c-1eb4-42ce-a2ba-67f3b7fce888`, and
  611 GUI tests passed in `4a89db31-2bf9-4476-a29a-d3fb6986dd94`. Five retained launcher scenes matched in light and
  dark in `2c4f1dfd-856f-410e-9b5c-028f9645c557`. No reference images needed updating.

  ```sh
  cargo nextest run -p farhelm-gui --lib -E 'test(=launcher::checkout_tests::switching_hosts_during_checkout_setup_waits_for_the_new_hosts_revision) | test(=launcher::checkout_tests::a_checkout_wake_retires_a_pending_folder_recheck) | test(=launcher::checkout_tests::a_checkout_wake_ignores_a_late_folder_save_reply)'
  cargo nextest run -p farhelm-gui --lib -E 'test(launcher) | test(checkout) | test(goldens)'
  python3 scripts/check-gui-goldens.py --scene 'launcher-managed-checkout*' --scene launcher-command-results --scene launcher-narrow --scene launcher-restart-with
  ```

- Reused three recorder runs from `769fc5c74f0a`. The 210 protocol, helm and supervisor tests
  (`4c528741-3800-4aaf-acc4-1798188a1b58`) and six Managed checkout integration tests
  (`37e8852d-279d-4115-887a-562ba2286df8`) still apply because the later fixes are desktop-only. The 47
  screenshot-registry tests (`688d5410-deed-4993-b152-809b0ab69258`) still apply because the registry and manifest
  stayed the same. The final screenshot comparison covered the changed scene setup.

  ```sh
  cargo nextest run -p farhelm-proto -p farhelm-helm -p farhelm-supervisor --lib -E 'test(github_repo) | test(checkout) | test(protocol_version) | test(migration) | test(request_modes) | test(the_reply_bound_answers_a_slow_host_with_a_timeout_and_keeps_the_work)'
  cargo nextest run -p farhelm --test e2e -E 'test(github_checkouts)'
  python3 -B scripts/test-gui-screenshot-batch.py
  ```

- Formatting, whitespace, workspace all-target Clippy and Farhelm binary Clippy passed on the final draft tree. The Rust
  test-delay check passed on `547b9944a5a5`. The subsequent fixture edit only combined two conditions and added no
  delays. Markdown format, changelog and instruction checks passed, and the website built 41 pages with valid internal
  links. Those inputs stayed the same after the checks. The site emitted its existing localization and 404 warnings. The
  instruction checker reported an existing archive heading warning.
- Skipped unrelated integration batches and a full screenshot recapture. The selected runs exercise the changed launch
  paths and the adjacent reference scenes.

## Review

The independent source and scope review gates passed after fixes, with no unresolved findings. Review caught two history
gaps during migration and session replacement. It also caught launch interruptions or repeated preview work while saving
a folder, changing hosts or waking a host. The checked stack includes the fixes.
