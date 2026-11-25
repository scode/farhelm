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

### Landing

#### What landed with it

Three plans landed this round, in this order: this one, through its two PRs and the landing fix
[#4944](https://github.com/scode/farhelm/pull/4944/changes); then hiding container hosts that a shared recipe cannot use
(#4934); then activating the previous terminal tab on close (#4940). Since this plan's PRs were written, main also
gained the whole Podman triage plan and the last ten PRs of the hooks and workspace trust triage, both landed in the
previous rounds.

Two of those touch what this plan changes:

- **Launcher test scenes.** This plan removes the screenshot scene that showed a repository search failing, since there
  is no search to fail any more, and changes the canned replies the screenshot test gives when the launcher asks for
  Managed checkout history and folder details. The Podman triage added a scene of New container after a refused checkout
  name, in the same screenshot test setup. Rebasing conflicted there; I merged them by hand, keeping both scenes'
  handling and leaving the removed scene out, and the full screenshot comparison passed on the result.
- **Session creation in the helm.** The Podman triage's check of a new container's checkout name and the hooks and trust
  plan's approved Claude trust choice are both in the code that creates a session in a fresh checkout. Neither reads the
  repository history, and the review found no interaction.

Neither of them changed the protocol version, so nothing needed renumbering. The TODO.md conflicts were the usual
entries removed on both sides; I kept both. A file-by-file comparison confirmed every other file's change is exactly
what the executor pushed.

The landing's review found no bug in this plan. It counted no blockers or should-fix items for it, and one small point,
fixed below.

#### Fixes made while landing

- **SPEC.md still named a repository search as slow-host work.** The paragraph on slow hosts gave "a repository search"
  as an example of host work that may take as long as the host makes it take. This plan removed that search, so #4944
  drops the example and changes nothing else.

#### Checks

This round landed three plans in queue order: the shared Managed checkout repository history, hiding container hosts
that a shared recipe cannot use, and activating the previous terminal tab on close. All the checks below ran once on the
three plans' PRs stacked in that order on main as it stood at the start of the landing (`3c39d3ff`), with the conflict
resolutions described in the Landing section of the Managed checkout repository history report; the stack's top commit
was `1d46ce6e`. Worker runs were on a separate arm64 Linux test worker; the rest ran locally on x86_64 Linux. The only
later change is the landing fix [#4944](https://github.com/scode/farhelm/pull/4944/changes), which drops a stale example
from one SPEC.md sentence; after it I ran `dprint check` on SPEC.md again, and it passed. That final tree is what main
now holds outside the plans queue.

- The workspace Rust suite, `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `f77a85f9-2e39-4531-b1e0-23ca4b4ca57e`): all 8660 tests passed. One test skipped itself at run time,
  `the_probed_launch_prefix_pins_every_load_bearing_flag`, because the worker's systemd user manager did not answer its
  probe at that moment; it checks the supervisor's launch-scope flags, which none of these plans touch, and it ran in
  the two previous rounds' runs, so its absence here is missing coverage unrelated to them. The plans changed the helm's
  store (a new schema step) and session creation, the protocol between helm and supervisor, the supervisor's request
  handlers, the recipe place list and recipe state, the Managed checkout end-to-end tests, and the GUI's launcher,
  Settings recipe cards and terminal tabs; the whole suite was the narrowest run that covered them and their callers.
- The desktop app's tests, `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run
  `b578690f-32d5-4cdc-9495-1e30c4d332e2`): all 184 passed. The desktop app builds the GUI crate these plans changed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings`,
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` and
  `cargo clippy -p farhelm-desktop --all-targets --features agent -- -D warnings`, on the test worker: passed.
- The desktop app's end-to-end smoke, `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run
  `dd8d7103-e0d0-4e3c-ba0a-b2c415fe833d`): 50 steps passed, one skipped by design on Linux (paste). It opens the
  launcher and drives terminal tabs and attach.
- The desktop app's screenshot comparison, `python3 scripts/check-gui-goldens.py`, all scenes, locally (recorder run
  `a2119b48-b97f-4fbb-abfd-0aade7959e9d`): passed, including the launcher scenes whose test setup the landing merged by
  hand and the container checkout refusal scene the Podman triage added.
- The test of the screenshot comparison tool and its scene list, `python3 -B scripts/test-gui-screenshot-batch.py`,
  locally (recorder run `71c56adc-6730-4bc8-885a-764e395c5806`): passed. The first plan removed a golden scene and its
  list entry.
- The docs website build, `bun install --frozen-lockfile && bun run build` in `website/`, locally: passed. Two docs
  pages changed (Start a session and Work in a session).
- `cargo fmt --all -- --check`, `python3 releasing/check-changelog.py format`, `python3 scripts/check-agents-md.py` (no
  errors; one warning, about an old citation in the triage archive, already on main),
  `python -B scripts/check-test-sleeps.py` and `dprint check`, locally: passed.

Reused: none. The executors' runs were focused selections on older bases, and everything they covered ran again above.

Skipped: an older supervisor connecting to the upgraded helm (the first plan's report asks for it during release
testing, since the protocol version changed); the workspace doctests, since no documentation example changed; a Mac run,
since nothing changed is Mac-specific; and CI, which runs only on demand.

#### What the report says that is no longer true

- Nothing. Its checks ran on an older base; everything they covered ran again on the landed stack.

#### Things you should know that the report does not say

- **Nothing in this landing tested an older supervisor against the upgraded helm.** The release testing step the report
  asks for (updating an existing host with an older supervisor, then launching a Managed checkout from a recent
  repository) is still the only check of that path.
