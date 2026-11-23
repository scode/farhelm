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

### Landing

#### What landed with it

The plan's four PRs (#4723, #4727, #4731, #4732) and one landing fix (#4733) landed first in a round with the
positional-hook-attribution plan, which landed right after it. Since the stack's base, main had gained only the
automatic host update fix (host-auto-update-after-2fa), which touches the helm's host connections and setup sign-in and
nothing this plan changed.

The two plans in this round were built at the same time and both reshape how the supervisor decides which process sent a
conversation report. This plan taught that code to recognize the custom agent reporter command and its shell wrapper
alongside the built-in hook command; the other moved the decision whether a recorded reporting process still holds the
session into shared code and put Grok and Codex on the same rule. Their commits conflicted in the supervisor's
attribution code, a code comment that maps where each agent's behavior lives, and one SPEC.md passage. The landing kept
both sides everywhere, and fixed the other plan's code where this plan's changes affected it (see that plan's landing
notes). The landing's two independent reviewers, one per plan, confirmed the result: custom agents' admission is
unchanged by the other plan, a custom report can only land in a custom agent's session and the reverse, and a launch
with reporting turned off still refuses reports however its registration changes.

The plan raises the wire protocol from 87 to 89, as its report says; nothing else on main or in this round changed it.

#### Fixes made while landing

- **A mistyped agent type got the wrong refusal.** SPEC.md requires `farhelm agent create --agent <value>` to refuse a
  value that is not an agent type with the list of agent types. Since custom agents, the CLI accepts any well-formed
  custom agent ID, because it cannot read the helm's registry, so `--agent cluade` reached the helm and was refused as
  "the custom agent is no longer registered", with no list. #4733 has the helm refuse an unregistered custom agent ID on
  a launch an agent starts (from `--agent` or a template) as neither a built-in type nor a registered custom agent,
  listing the built-in types. A deleted registration gets the same refusal, since the helm keeps no record of deletions;
  the end-to-end test that deletes one checks the new wording.
- **Two helm database tests failed.** They rewind a new database to the old schema 26 by undoing every later schema
  step, and the plan's new custom agents table was not undone. The executor's filtered runs never selected them; the
  landing's full run did. #4733 drops the table first.
- **The desktop test harness's own tests failed** on the new custom launch type, which the harness reads from the
  source. #4733 leaves the custom type out of the scenario harness list, as the custom permission already is, since a
  staged scenario has no way to name a registered custom agent.
- #4733 also corrects text the plan left stale: SPEC.md's sentence on what a named Restart does with a custom agent's
  recorded conversation (it reads "keeps the old capture until a report from the new launch replaces it"), a supervisor
  refusal that still said agents cannot start a session under a chosen conversation, and an installer comment about the
  reporter's silenced output. It adds a note about the refusal to the plan's changelog fragment.

#### Checks

The two plans touch the protocol, the supervisor's report attribution, the helm's launches, registry and CLI, the
desktop window's Settings and launcher, the installer's launcher script and the docs site, and each executor ran
filtered selections. So the landing ran the whole workspace suite, every lint, the desktop app's own tests (the app
embeds the changed window), the installer test and the end-to-end smoke on the two plans stacked on main, with every
reference picture, the website build and the source checks locally. The worker runs were on a separate arm64 Linux test
worker; the rest ran locally on x86_64 Linux.

On both plans stacked on main, with the conflict resolutions and compile fixes:

- `cargo nextest run --workspace --exclude farhelm-desktop`, on the test worker (recorder run
  `adc951be-a12e-44f5-9490-806b28f11ad4`): 8508 of 8511 passed and none printed a runtime `SKIPPED`. Two of the failures
  were the helm's schema 26 migration tests that custom agents broke (fixed in #4733, see its notes). The third was the
  helm's Tensorlake test of carrying a Codex sign-in choice into environment setup, which found a Codex credential
  marked cleared where it expected none. Neither plan touches that flow, and the same test failed once during the
  host-auto-update-after-2fa plan's execution on a tree without either plan. It passed 20 of 20 repeated runs alone on
  this tree, locally on x86_64 rather than on the arm64 worker where it failed (recorder repetition batch
  `2ebb6eaf-0b96-4280-9ac5-17a22c8ac47a`), and again on the worker in the run after the fixes below. It looks like a
  latent flake that shows only under load. The landing did not record it in FLAKES.md, which takes its own change;
  recording it is a follow-up.
- `cargo nextest run -p farhelm-desktop`, on the test worker (recorder run `8dce2b68-35aa-4ad4-a8ee-e51508e3ac5f`): all
  184 passed.
- `cargo clippy --all-targets -- -D warnings`, `cargo clippy -p farhelm --bins -- -D warnings` and the desktop app's
  `cargo clippy -p farhelm-desktop --all-targets -- -D warnings` with and without `--features agent`, on the test
  worker: passed.
- `bash scripts/test-install-sh.sh --farhelm target/debug/farhelm`, on the test worker, run directly as the validation
  guide lists it rather than through the recorder: 547 checks, none failed.
- `python3 -B scripts/gui-e2e-smoke.py`, on the test worker (recorder run `bd07df46-6f7f-449c-89cf-6fe615c8bf44`): of
  its 50 steps, 49 passed and one was skipped as always on Linux (paste, which the headless Linux desktop session the
  smoke runs in cannot provide).
- `python3 scripts/check-gui-goldens.py`, locally (recorder run `9f6e7283-22b0-4a28-8465-fa845314e194`): every reference
  picture matched, including custom agents' new launcher, Settings and approval scenes.
- `python3 -B scripts/test-gui-agent.py`, locally (a self-test the validation guide lists without the recorder): failed
  on the new custom launch type (fixed in #4733; see the custom-agents landing notes).
- `cargo check -p farhelm-supervisor --tests`, locally, at each of the positional plan's first three commits and at the
  top of the stack: compiled, so each of that plan's commits builds on its own after the landing's fixes.
- `python3 -B scripts/test-gui-screenshot-batch.py`, `sh -n scripts/install.sh` with `shellcheck`,
  `python3 scripts/check-agents-md.py`, the website build (`bun install --frozen-lockfile` and `bun run build`),
  `cargo fmt --all -- --check`, `python -B scripts/check-test-sleeps.py` (in its pinned interpreter), `dprint check` and
  `python3 releasing/check-changelog.py format`, locally: passed.

After the landing fixes (#4733 and #4734):

- `cargo nextest run -p farhelm-helm -p farhelm --lib --test e2e` selecting the helm's library and the agent listing
  end-to-end module, whose test checks the new refusal, on the test worker (recorder run
  `89e4e6ca-903f-4197-9fa9-479aada524d8`): all 1891 passed, including both migration tests and the Tensorlake test, and
  none printed a runtime `SKIPPED`.
- `cargo clippy --all-targets -- -D warnings` and `cargo clippy -p farhelm --bins -- -D warnings`, on the test worker:
  passed.
- `python3 -B scripts/test-gui-agent.py`, `sh -n scripts/install.sh` with `shellcheck`, the website build,
  `cargo fmt --all -- --check`, the sleep check, `dprint check` and the changelog format check, locally: passed.

Reused for the fixes: the rest of the workspace suite, the desktop tests, the installer test, the smoke and the
reference pictures above, since the fixes change one helm refusal, a test fixture, two scripts' launch type lists, a
supervisor message and documentation. Skipped: macOS, where no Mac was used, so custom agents' new Settings and launcher
screens are unchecked there; the real-agent drift tests for Grok and Codex, which the positional plan's executor ran and
nothing at landing changed their behavior; CI, which runs only on demand.

#### Things you should know that the report does not say

- **Agents cannot find out which custom agents exist.** The agent instructions tell agents to use `--agent <id>`, but no
  `farhelm agent` command lists registered custom agents, as there is for templates and hosts. An agent can only use one
  it is told about. Two cheap options: add such a listing, or have the landing's new unknown-agent refusal also list the
  registered custom agents.
- **A future built-in agent can lock out a custom agent with the same name.** IDs matching a built-in agent are
  reserved, and every launch and save re-checks the stored registration. If Farhelm later ships a built-in agent whose
  name a user already registered, that registration could no longer be launched or edited, and `--agent <name>` would
  pick the built-in. The reviewer's options: check reserved names only when an ID is first registered, or move such a
  registration to a new ID when the built-in ships. Either is worth deciding before such an agent ships.
- **A mistyped `--effort` for a built-in agent no longer lists the choices.** The CLI now accepts any effort word for
  every agent so custom agents can use their own; the helm refuses an unknown built-in effort without the list. SPEC.md
  requires the list only for agent types.
- **`--conversation` and recipes disagree.** The CLI refuses `--conversation` with `--recipe`, but a template that puts
  the session in a recipe passes the conversation through and it works. SPEC.md does not list recipes among the launches
  that refuse a conversation, so either the CLI's refusal goes or SPEC.md gains it.
- **One unreadable custom agent row would refuse every agent create**, built-in agents included, and the Settings list.
  That needs a hand-edited database, so the risk is low.
- The changelog fragment's second paragraph reads as if **More options → Conversation ID** were new for every agent; for
  built-in agents the new part is `--conversation` on the CLI. SPEC.md's list of conversation selector labels does not
  mention custom agents.
