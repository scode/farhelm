## What this was about

A coding agent without a built-in integration could run as a command, but its reusable launch setup and conversation
tracking depended on support Farhelm shipped. The plan adds a registry in **Settings → Custom agents** and a small
contract for the executable or wrapper. A saved registration appears in the launcher, templates and CLI. An agent that
supports resumption and reports its conversation ID can use **Restart**. A known ID can also start a new session on that
conversation.

## Things you should know

- A registration uses the same executable name or path on every host and environment. Its display name can change while
  the ID used by templates and CLI commands stays fixed. Model and effort are typed strings, and custom agents always
  count as YOLO under the host's existing approval rules. Initial prompts are unavailable in this version.
- A running session keeps its compiled commands and reporting capability after a registration changes. Plain **Restart**
  uses the saved resume command and captured ID. The list falls back to the registration ID if it is deleted. Rebuilding
  a launch from a deleted registration requires restoring it, or choosing generic **Custom** for a Command launch whose
  agent type can be changed.
- **Supports session resumption** and **Supports session reporting** are independent. The first lets the launcher pass a
  known ID even before an agent has reported. The second lets the agent report a conversation to use for Restart. A
  declared custom Command carries its own resume command and can report without a registration.
- The offline `farhelm custom-agent spec` command embeds the same contract as the website. The first contract version
  has no stability guarantee. The documentation keeps the approved prose and adds notes about named Restart with,
  invalid report arguments and the possible order of hook-log lines.
- The helm and its supervisors must run a build that speaks protocol 89. After you upgrade the helm, older remote
  supervisors refuse the connection and their Hosts rows show that an update is required. If a remote supervisor
  upgrades first, its row shows **Too new** until the helm upgrades.

## Open questions and possible follow-ups

None.

## Pull requests

- Accept and attribute conversation reports from custom agents.
  [#4723](https://github.com/scode/farhelm/pull/4723/changes)
- Store registrations, compile custom launches and accept named starts on the CLI.
  [#4727](https://github.com/scode/farhelm/pull/4727/changes)
- Register and launch custom agents in the desktop app. [#4731](https://github.com/scode/farhelm/pull/4731/changes)
- Document the contract, embed it in the offline CLI and clear the TODO item.
  [#4732](https://github.com/scode/farhelm/pull/4732/changes)

## Checks

- The reporter selection passed all 1,956 tests on `00d56dd5d527` (run `bfb8e12b-7e33-41b5-b6a6-2e759a1a0842`). That
  includes four real-process reporter cases covering nested-report attribution, wrapped launches and the no-session
  path. Reused the result after rebasing over desktop styling and update reminders because the tested behavior stayed
  the same. Default-member all-targets Clippy and binary Clippy passed.

  ```text
  cargo nextest run --locked -p farhelm-proto -p farhelm-supervisor -p farhelm -p farhelm-helm -p farhelm-gui
    --lib --bin farhelm --test e2e --test version_lock_forwarder
    -E 'package(farhelm-proto) | (package(farhelm-supervisor) & test(/^(agent_kind|procs|service::(report_files|core))::/)) | (package(farhelm) & (test(/^(hook|hook_identity)::/) | binary(version_lock_forwarder))) | (package(farhelm-helm) & test(/^(launches|templates|sessions::workspace_trust_tests)::/)) | (package(farhelm-gui) & test(/^(templates|session_list::row_marks|approvals|launcher|agent_startup|harness)::/))'
  ```

- The catalog and CLI selection ran 660 tests on `a9352f663f90` (run `bf2e4569-4545-48ba-8254-d0ec836f72a5`). There were
  658 passes and two failures in the CLI help output and the first UI snapshot. Both passed after their expected values
  were updated (run `e6990259-d984-4c20-bdb1-20c603b71441`). Four focused cases also passed on `a9352f663f90` (run
  `24cfc989-5cb7-44f0-946e-c551689e9d3a`). They cover repeated named launches, restarting after deleting a registration,
  sandbox launch admission and template approval and display. Reused the passing cases because the two updated
  expectations were the only changes. Default-member all-targets Clippy and binary Clippy passed.

  ```text
  cargo nextest run --locked -p farhelm-helm -p farhelm-helm-client -p farhelm-proto -p farhelm-supervisor -p farhelm
    --lib --bin farhelm --test agent_cli --test spawn_cli --test e2e
    -E 'package(farhelm-helm-client) | (package(farhelm-helm) & test(/^(custom_agents|launches|templates|store|ui_channel|agent_launch|agent_requests)::/)) | (package(farhelm-proto) & test(/(protocol_version_is_pinned|named_conversation|custom_effort)/)) | (package(farhelm-supervisor) & test(/(a_request_fingerprint_beside_a_fresh_checkout_is_refused|an_agent_create_is_matched_by_its_request_fingerprint)/)) | (package(farhelm) & (test(/^(agent_instructions|structured_launches)::/) | test(custom_create_keeps_the_named_start) | binary(agent_cli) | binary(spawn_cli)))'
  ```

  The focused filter was
  `test(a_custom_named_start_uses_the_catalog) | test(a_recipe_create_carries_the_admitted_custom_agent_snapshot) | test(a_template_card_names_the_custom_agent_registration) | test(a_templates_sets_cell_reads_as_its_flags)`.
  The retry selected
  `test(=agent_instructions::tests::a_creating_verb_renders_its_real_command_line) | test(=ui_channel::tests::the_snapshot_on_open_holds_every_collection_as_served)`.

- The desktop selection passed all 1,926 tests on `4a312d25e85e` (run `c10e6a16-f886-4362-9f03-672d2e76d2fd`). The draft
  has the same tree. GUI and desktop all-targets Clippy, desktop Clippy with the agent feature, the 120 GUI lint
  demonstrations and crate layering passed. The generated SVG matched its source.

  ```text
  cargo nextest run --locked -p farhelm-gui -p farhelm-desktop -p farhelm-proto -p farhelm-helm
    -E 'package(farhelm-desktop) | (package(farhelm-gui) & test(/^(assets|harness|marks|settings|launcher|templates|session_list)::/)) | (package(farhelm-proto) & test(/^(launch|launcher|session_launch)::/)) | (package(farhelm-helm) & test(/^(templates|agent_launch|custom_agents)::/))'
  ```

  The full Linux reference update and comparison passed with `python3 scripts/check-gui-goldens.py --update -j 3` and
  `python3 scripts/check-gui-goldens.py -j 3` (runs `65a94b2e-bbfc-4e05-8dff-7d8b1154d9c6` and
  `c384c49f-b2dc-406c-a5f7-7f8d22476bfa`). The four new images were inspected in both themes.
  `python3 -B scripts/test-gui-screenshot-batch.py SceneList.test_the_committed_scene_list_is_well_formed` passed in run
  `7aaa10dc-5b9d-4ad4-af64-68237ee6fdce`.
- `cargo test --locked -j 3 -p farhelm --bin farhelm custom_agent_spec_keeps_the_contract_and_omits_site_markup -- --exact`
  passed in run `27e7c887-410e-4ad9-ad56-cbf48519bb99`. It checks that the printed contract retains the changelog and
  stability note and omits website-specific markup. The same code reached the draft after a changelog-only edit. The
  local debug build printed the contract with `farhelm custom-agent spec` outside a session (run
  `0210c301-9b94-4db2-a742-5ec386d17073`). In `website/`, `bun install --frozen-lockfile` and `bun run build` succeeded,
  producing 41 pages with valid links. Markdown formatting, the instruction checker, the changelog format and test-delay
  checks passed.
- Skipped the full workspace battery because the retained selections exercise the changed behavior across crates. Linux
  rendering and local wrapper fixtures provide the desktop and reporting evidence. macOS rendering and real third-party
  wrappers were outside those selections. Hosted CI was not requested.

## Review gate

Each of the four changes received an independent Sol review. The findings led to preserving the admitted command through
environment setup, handling repeated named-launch requests consistently, retaining edits when a registration disappears,
refusing to silently replace a named start with a fresh one and clarifying the offline contract. The executor resolved
and verified all review findings before opening the drafts. Commit wording passed separate cold reads.
